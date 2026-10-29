//! Pixel layers stacked back to front, rendered with half-blocks.
//!
//! Each terminal cell holds two pixels (`▀`: fg = top, bg = bottom), so
//! layers overlap at half-row resolution. Each frame, every layer draws
//! once into a pixel canvas — `None` is transparent and lets the layers
//! behind show through — and is composited onto the ones behind with its
//! own `BlendMode` and opacity (default: Normal, opaque).
//!
//! Two ways to draw a layer:
//! - `Layer::new(|px| …)` answers one pixel at a time. Easy; fine for
//!   cheap per-pixel maths.
//! - `Layer::paint(|canvas| …)` draws the whole frame at once — compute
//!   per-column or per-frame values once, stamp sprites, paint in order.

use super::easing::Easing;
use super::effects::BlendMode;
use super::framebuffer::{Cell, Effect, FrameBuffer, FPS};
use crate::color::Color;

/// One pixel query handed to a layer's closure.
#[derive(Debug, Clone, Copy)]
pub struct Px {
    /// Horizontal position in the layer's own space — screen x minus the
    /// layer's current motion offset. Fractional while swinging.
    pub x: f64,
    /// Pixel row (two per terminal row), top = 0.
    pub y: usize,
    /// Screen pixel column, unaffected by motion.
    pub sx: usize,
    pub frame: usize,
}

impl Px {
    /// 2×2 ordered dither of `a` and `b`, locked to the screen grid.
    pub fn dither(&self, a: Color, b: Color, level: u8) -> Color {
        dither(a, b, level, self.sx, self.y)
    }
}

/// 2×2 ordered dither of `a` and `b` at screen pixel (x, y).
/// `level` 0..=4 = how many of each 2×2 block's pixels take `b`.
pub fn dither(a: Color, b: Color, level: u8, x: usize, y: usize) -> Color {
    const BAYER: [[u8; 2]; 2] = [[0, 2], [3, 1]];
    if BAYER[y & 1][x & 1] < level { b } else { a }
}

/// One layer's drawing surface for one frame, in screen pixels.
pub struct Canvas<'a> {
    pub width: usize,
    pub height: usize,
    pub frame: usize,
    /// The layer's current motion offset: screen x = layer x + offset.
    pub offset: f64,
    px: &'a mut [Option<Color>],
}

impl Canvas<'_> {
    /// Paint screen pixel (x, y); out-of-bounds is ignored.
    pub fn set(&mut self, x: i64, y: i64, color: Color) {
        if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
            self.px[y as usize * self.width + x as usize] = Some(color);
        }
    }
}

type PixelFn = Box<dyn Fn(Px) -> Option<Color> + Send>;
type PaintFn = Box<dyn Fn(&mut Canvas) + Send>;

enum Draw {
    Pixel(PixelFn),
    Paint(PaintFn),
}

/// A single layer: how it draws, plus optional back-and-forth motion.
pub struct Layer {
    draw: Draw,
    amplitude: f64,
    cycle_secs: f64,
    easing: Easing,
    phase: f64,
    mode: BlendMode,
    opacity: f64,
}

impl Layer {
    /// A layer drawn one pixel at a time.
    pub fn new(pixel: impl Fn(Px) -> Option<Color> + Send + 'static) -> Self {
        Self::with(Draw::Pixel(Box::new(pixel)))
    }

    /// A layer that paints its whole canvas once per frame.
    pub fn paint(paint: impl Fn(&mut Canvas) + Send + 'static) -> Self {
        Self::with(Draw::Paint(Box::new(paint)))
    }

    fn with(draw: Draw) -> Self {
        Self {
            draw,
            amplitude: 0.0,
            cycle_secs: 0.0,
            easing: Easing::EaseInOut,
            phase: 0.0,
            mode: BlendMode::Normal,
            opacity: 1.0,
        }
    }

    /// How this layer combines with the layers behind it.
    pub fn blend(mut self, mode: BlendMode) -> Self {
        self.mode = mode;
        self
    }

    /// Fade this layer: 0 = invisible, 1 = full strength (default).
    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Sway horizontally ±`amplitude` pixels, one full there-and-back
    /// every `cycle_secs`, each sweep shaped by `easing`.
    pub fn swing(mut self, amplitude: f64, cycle_secs: f64, easing: Easing) -> Self {
        self.amplitude = amplitude;
        self.cycle_secs = cycle_secs.max(0.0);
        self.easing = easing;
        self
    }

    /// Offset into the swing cycle, as a fraction of it. Staggering
    /// phases across layers makes the motion ripple through the stack.
    pub fn phase(mut self, fraction: f64) -> Self {
        self.phase = fraction;
        self
    }

    fn offset(&self, frame: usize) -> f64 {
        if self.cycle_secs == 0.0 {
            return 0.0;
        }
        let t = frame as f64 / FPS / self.cycle_secs;
        self.amplitude * self.easing.swing(t + self.phase)
    }
}

/// A stack of `Layer`s, `rows` terminal rows tall (2 × rows pixels),
/// as wide as the scene. Add back to front.
pub struct Layers {
    layers: Vec<Layer>,
    rows: usize,
}

impl Layers {
    pub fn new(rows: usize) -> Self {
        Self { layers: Vec::new(), rows }
    }

    /// Push a layer in front of the ones already added.
    pub fn layer(mut self, layer: Layer) -> Self {
        self.layers.push(layer);
        self
    }

    /// Draw every layer once and composite back to front.
    fn composite(&self, width: usize, height: usize, frame: usize) -> Vec<Option<Color>> {
        let mut acc: Vec<Option<Color>> = vec![None; width * height];
        let mut scratch: Vec<Option<Color>> = vec![None; width * height];
        for layer in &self.layers {
            let offset = layer.offset(frame);
            scratch.fill(None);
            match &layer.draw {
                Draw::Pixel(f) => {
                    for y in 0..height {
                        for sx in 0..width {
                            scratch[y * width + sx] = f(Px { x: sx as f64 - offset, y, sx, frame });
                        }
                    }
                }
                Draw::Paint(f) => f(&mut Canvas { width, height, frame, offset, px: &mut scratch }),
            }
            for (under, over) in acc.iter_mut().zip(&scratch) {
                if let Some(o) = *over {
                    *under = Some(match *under {
                        Some(u) => layer.mode.mix(u, o, layer.opacity),
                        None => o,
                    });
                }
            }
        }
        acc
    }
}

impl Effect for Layers {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let rows = buf.height.min(self.rows);
        let width = buf.width;
        let px = self.composite(width, rows * 2, frame);
        for row in 0..rows {
            for x in 0..width {
                let top = px[row * 2 * width + x];
                let bottom = px[(row * 2 + 1) * width + x];
                let cell = match (top, bottom) {
                    (Some(t), Some(b)) => Cell::with_bg('▀', t, b),
                    (Some(t), None) => Cell::new('▀', t),
                    (None, Some(b)) => Cell::new('▄', b),
                    (None, None) => continue,
                };
                buf.set(x, row, cell);
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (0, self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Color = Color { r: 255, g: 0, b: 0 };
    const B: Color = Color { r: 0, g: 0, b: 255 };

    fn render(layers: &Layers, frame: usize) -> FrameBuffer {
        let mut buf = FrameBuffer::new(8, layers.rows);
        layers.render(&mut buf, frame);
        buf
    }

    #[test]
    fn front_layer_wins_and_transparency_shows_through() {
        // Back fills everything; front covers only the bottom pixel row.
        let layers = Layers::new(1)
            .layer(Layer::new(|_| Some(A)))
            .layer(Layer::new(|p| (p.y == 1).then_some(B)));
        let cell = render(&layers, 0).get(0, 0);
        assert_eq!((cell.ch, cell.color, cell.bg), ('▀', A, Some(B)));
    }

    #[test]
    fn half_covered_cell_uses_terminal_background() {
        let layers = Layers::new(1).layer(Layer::new(|p| (p.y == 1).then_some(B)));
        let cell = render(&layers, 0).get(0, 0);
        assert_eq!((cell.ch, cell.color, cell.bg), ('▄', B, None));
    }

    #[test]
    fn swing_shifts_layer_space() {
        // Quarter cycle in: offset = +amplitude, so screen x 0 samples x = -2.
        let layers = Layers::new(1).layer(
            Layer::new(|p| (p.x < -1.5).then_some(A)).swing(2.0, 4.0, Easing::Linear),
        );
        assert_eq!(render(&layers, 0).get(0, 0).ch, ' ');
        assert_eq!(render(&layers, 60).get(0, 0).color, A);
    }

    #[test]
    fn blend_mode_layers_mix_with_whats_behind() {
        let layers = Layers::new(1)
            .layer(Layer::new(|_| Some(A)))
            .layer(Layer::new(|_| Some(B)).blend(BlendMode::Average))
            .layer(Layer::new(|p| (p.y == 1).then_some(B)).opacity(0.5));
        let cell = render(&layers, 0).get(0, 0);
        let avg = BlendMode::Average.mix(A, B, 1.0);
        assert_eq!(cell.color, avg);
        assert_eq!(cell.bg, Some(Color::lerp_rgb(avg, B, 0.5)));
    }

    #[test]
    fn paint_layers_stack_in_order() {
        let layers = Layers::new(1)
            .layer(Layer::new(|_| Some(A)))
            .layer(Layer::paint(|c| c.set(0, 1, B)));
        let cell = render(&layers, 0).get(0, 0);
        assert_eq!((cell.ch, cell.color, cell.bg), ('▀', A, Some(B)));
        assert_eq!(render(&layers, 0).get(1, 0).bg, Some(A));
    }

    #[test]
    fn dither_levels() {
        let count = |level| {
            (0..2)
                .flat_map(|y| (0..2).map(move |sx| (sx, y)))
                .filter(|&(sx, y)| Px { x: 0.0, y, sx, frame: 0 }.dither(A, B, level) == B)
                .count()
        };
        for level in 0..=4u8 {
            assert_eq!(count(level), level as usize);
        }
    }
}
