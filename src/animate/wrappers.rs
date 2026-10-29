//! Wrappers that change how any effect looks or plays, without touching
//! the effect itself:
//!
//! - `Recolor` — run an effect, then repaint its glyphs through your own
//!   palette (by brightness, or by hue).
//! - `Padded` — give an effect empty room around it, so motion that
//!   leaves its own bounds (DYCP dips, FLD bounces) isn't cut off.
//! - `TimeScale` — play an effect faster or slower.
//!
//! Each is also a method on every effect via `EffectExt`:
//! `.recolor(palette)`, `.recolor_hue(palette)`, `.pad(top, right,
//! bottom, left)`, `.time_scale(k)`.

use super::framebuffer::{Effect, FrameBuffer};
use crate::color::Color;

/// How `Recolor` reads a glyph's colour.
#[derive(Debug, Clone, Copy, PartialEq)]
enum By {
    /// Dark → start of the palette, light → end.
    Luma,
    /// Hue angle → position around a cyclic palette.
    Hue,
}

/// Runs an effect, then repaints every glyph through `palette` — keeping
/// the effect's motion and light, in your colours. Backgrounds and blank
/// cells are left as they are.
pub struct Recolor {
    inner: Box<dyn Effect>,
    palette: Vec<Color>,
    by: By,
}

impl Recolor {
    /// Recolour by brightness: dark glyphs take the palette's first
    /// colour, bright ones its last.
    pub fn new(inner: impl Effect, palette: Vec<Color>) -> Self {
        Self { inner: Box::new(inner), palette, by: By::Luma }
    }

    /// Recolour by hue: 0°–360° maps across the palette, so a palette that
    /// ends where it starts keeps colour cycles seamless.
    pub fn hue(inner: impl Effect, palette: Vec<Color>) -> Self {
        Self { inner: Box::new(inner), palette, by: By::Hue }
    }
}

impl Effect for Recolor {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.inner.render(buf, frame);
        if self.palette.is_empty() {
            return;
        }
        let last = (self.palette.len() - 1) as f64;
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch == ' ' {
                    continue;
                }
                let t = match self.by {
                    By::Luma => cell.color.luma(),
                    By::Hue => cell.color.to_hsv().0 / 360.0,
                };
                buf.set_color(x, y, self.palette[(t.clamp(0.0, 1.0) * last).round() as usize]);
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        self.inner.size()
    }
}

/// An effect with empty room around it. The effect draws from
/// (`left`, `top`) and may use everything to its right and below —
/// padding included — so motion that dips or drifts past its own size
/// (DYCP, Wind) isn't cut off. For motion that also goes up or left (FLD),
/// pad the effect *inside* it: `Fld::new(text.pad(2, 0, 2, 0))`.
pub struct Padded {
    inner: Box<dyn Effect>,
    top: usize,
    right: usize,
    bottom: usize,
    left: usize,
}

impl Padded {
    /// Room in rows/columns: top, right, bottom, left (CSS order).
    pub fn new(inner: impl Effect, top: usize, right: usize, bottom: usize, left: usize) -> Self {
        Self { inner: Box::new(inner), top, right, bottom, left }
    }
}

impl Effect for Padded {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut sub = FrameBuffer::new(buf.width.saturating_sub(self.left), buf.height.saturating_sub(self.top));
        self.inner.render(&mut sub, frame);
        for y in 0..sub.height {
            for x in 0..sub.width {
                let cell = sub.get(x, y);
                if cell.ch != ' ' || cell.bg.is_some() {
                    buf.set(x + self.left, y + self.top, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        let (w, h) = self.inner.size();
        (w + self.left + self.right, h + self.top + self.bottom)
    }
}

/// Plays an effect `k` times as fast: 2.0 doubles the speed, 0.5 halves
/// it. The effect sees frame `round(frame × k)`.
pub struct TimeScale {
    inner: Box<dyn Effect>,
    k: f64,
}

impl TimeScale {
    pub fn new(inner: impl Effect, k: f64) -> Self {
        Self { inner: Box::new(inner), k: k.max(0.0) }
    }
}

impl Effect for TimeScale {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.inner.render(buf, (frame as f64 * self.k).round() as usize);
    }

    fn size(&self) -> (usize, usize) {
        self.inner.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animate::effects::{text, Dycp};
    use crate::animate::framebuffer::EffectExt;

    const GREY: fn(u8) -> Color = |v| Color::new(v, v, v);

    /// Writes the frame it was asked for into every cell's red channel.
    struct FrameProbe;
    impl Effect for FrameProbe {
        fn render(&self, buf: &mut FrameBuffer, frame: usize) {
            buf.set(0, 0, crate::animate::Cell::new('#', Color::new((frame % 256) as u8, 0, 0)));
        }
        fn size(&self) -> (usize, usize) {
            (1, 1)
        }
    }

    fn render(fx: &dyn Effect, frame: usize) -> FrameBuffer {
        let (w, h) = fx.size();
        let mut buf = FrameBuffer::new(w, h);
        fx.render(&mut buf, frame);
        buf
    }

    #[test]
    fn recolor_maps_brightness_onto_the_palette() {
        let palette = vec![Color::new(0, 0, 255), Color::new(255, 0, 0)];
        let dark = render(&text("a", GREY(0)).recolor(palette.clone()), 0);
        let light = render(&text("a", GREY(255)).recolor(palette.clone()), 0);
        assert_eq!(dark.get(0, 0).color, palette[0]);
        assert_eq!(light.get(0, 0).color, palette[1]);
        assert_eq!(light.get(0, 0).ch, 'a');
    }

    #[test]
    fn recolor_hue_maps_hue_around_a_cyclic_palette() {
        let palette: Vec<Color> = (0..=360).map(|h| GREY((h * 255 / 360) as u8)).collect();
        let cyan = render(&text("a", Color::new(0, 255, 255)).recolor_hue(palette.clone()), 0);
        assert_eq!(cyan.get(0, 0).color, palette[180]);
    }

    #[test]
    fn recolor_leaves_blank_cells_and_backgrounds_alone() {
        let plain = render(&text("a b", Color::new(10, 20, 30)), 0);
        let buf = render(&text("a b", Color::new(10, 20, 30)).recolor(vec![Color::new(255, 0, 0)]), 0);
        assert_eq!(buf.get(1, 0), plain.get(1, 0), "blank cell untouched");
        assert_eq!(buf.get(0, 0).bg, plain.get(0, 0).bg, "background untouched");
        assert_eq!(buf.get(0, 0).color, Color::new(255, 0, 0));
    }

    #[test]
    fn pad_grows_the_size_and_shifts_the_content() {
        let fx = text("ab", GREY(200)).pad(2, 1, 3, 4);
        assert_eq!(fx.size(), (2 + 1 + 4, 1 + 2 + 3));
        let buf = render(&fx, 0);
        assert_eq!(buf.get(4, 2).ch, 'a');
        assert_eq!(buf.get(5, 2).ch, 'b');
        assert_eq!(buf.get(0, 0).ch, ' ');
    }

    #[test]
    fn padding_below_catches_what_dycp_would_lose() {
        // DYCP dips letters below its own text; with room below nothing is
        // cut off — the same ink shows as in a far taller buffer.
        let ink = |fx: &dyn Effect, extra: usize, frame| {
            let (w, h) = fx.size();
            let mut buf = FrameBuffer::new(w, h + extra);
            fx.render(&mut buf, frame);
            (0..h + extra).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| buf.get(x, y).ch != ' ').count()
        };
        let bare = Dycp::new("WAVE WAVE");
        let padded = Dycp::new("WAVE WAVE").pad(0, 0, 3, 0);
        let frames: Vec<usize> = (0..600).step_by(7).collect();
        assert!(frames.iter().any(|&f| ink(&bare, 0, f) < ink(&bare, 20, f)));
        for &f in &frames {
            assert_eq!(ink(&padded, 0, f), ink(&padded, 20, f), "frame {f}");
        }
    }

    #[test]
    fn time_scale_plays_faster_and_slower() {
        let red = |fx: &dyn Effect, frame| render(fx, frame).get(0, 0).color.r;
        assert_eq!(red(&FrameProbe.time_scale(6.0), 10), 60);
        assert_eq!(red(&FrameProbe.time_scale(0.5), 10), 5);
        assert_eq!(red(&FrameProbe.time_scale(1.0), 7), 7);
        assert_eq!(FrameProbe.time_scale(3.0).size(), (1, 1));
    }
}
