//! Native framebuffer effect implementations.
//!
//! Each effect implements the `Effect` trait: takes a `FrameBuffer` and a frame
//! number, writes `(char, Color)` cells directly. No ANSI strings, no parsing.

use crate::color::Color;
use super::framebuffer::{Cell, Effect, EffectExt, On, FrameBuffer};
#[cfg(not(target_arch = "wasm32"))]
use super::framebuffer::AnimationHandle;

/// Adds `.spawn()`, `.run()`, `.frame()` to effects that carry their own text.
macro_rules! impl_text_effect_convenience {
    ($ty:ty) => {
        impl $ty {
            /// Render a single frame to an ANSI string.
            pub fn frame(&self, frame: usize) -> String {
                let (w, h) = <Self as Effect>::size(self);
                let mut buf = FrameBuffer::new(w.max(1), h.max(1));
                <Self as Effect>::render(self, &mut buf, frame);
                buf.to_ansi_string()
            }

            /// Spawn in a terminal area. Runs until `.stop()` or `.fade_out()`.
            #[cfg(not(target_arch = "wasm32"))]
            pub fn spawn(self) -> AnimationHandle {
                let (w, h) = <Self as Effect>::size(&self);
                super::framebuffer::spawn_effect(self, w.max(1), h.max(1), 1.0)
            }

            /// Run in a terminal area for `seconds`, then stop.
            #[cfg(not(target_arch = "wasm32"))]
            pub async fn run(self, seconds: f64) {
                let (w, h) = <Self as Effect>::size(&self);
                super::framebuffer::run_effect(
                    self, w.max(1), h.max(1),
                    std::time::Duration::from_secs_f64(seconds), 1.0,
                ).await;
            }
        }
    };
}

/// Helper: parse text into a Vec of char-lines.
fn text_to_lines(text: &str) -> Vec<Vec<char>> {
    text.split('\n').map(|l| l.chars().collect()).collect()
}

/// Helper: compute (width, height) from char-lines.
fn chars_size(chars: &[Vec<char>]) -> (usize, usize) {
    let h = chars.len();
    let w = chars.iter().map(|l| l.len()).max().unwrap_or(0);
    (w, h)
}

// ── Static text ──

/// Static color — sets all non-space cells to one color. No animation.
pub struct Solid(pub Color);

impl Effect for Solid {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch != ' ' {
                    buf.set_color(x, y, self.0);
                }
            }
        }
    }
}

/// Static colored text for use in scenes.
pub fn text(s: &str, color: Color) -> On<Solid> {
    EffectExt::on(Solid(color), s)
}

// ── Rainbow ──

/// Rainbow HSV hue rotation. Colors whatever text is in the buffer.
pub struct Rainbow;

impl Rainbow {
    pub fn new() -> Self { Self }
    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self, text) }
}

impl Effect for Rainbow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let hue_offset = (frame * 5 % 360) as f64;
        let w = buf.content_width();

        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
                let hue = (hue_offset + (x as f64 / w as f64) * 360.0) % 360.0;
                buf.set_color(x, y, Color::from_hsv(hue, 1.0, 1.0));
            }
        }
    }
}

// ── Glow ──

/// Sweeping spotlight that travels through a gradient palette.
pub struct Glow {
    palette: Vec<Color>,
}

impl Glow {
    pub fn new() -> Self {
        Self { palette: crate::presets::rainbow().palette(256) }
    }

    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self::new(), text) }

    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.palette = palette;
        self
    }
}

impl Effect for Glow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let w = buf.content_width();
        let pal = &self.palette;
        if pal.is_empty() { return; }

        let spotlight = (frame as f64 * 0.02).sin() * 0.5 + 0.5;

        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
                let pos = x as f64 / w as f64;
                let dist = (pos - spotlight).abs();
                let brightness = (1.0 - dist * 3.0).max(0.15);

                let idx = (pos * (pal.len() - 1) as f64).min((pal.len() - 1) as f64);
                let lo = idx.floor() as usize;
                let hi = (lo + 1).min(pal.len() - 1);
                let frac = idx - lo as f64;
                let base = Color::lerp_rgb(pal[lo], pal[hi], frac);
                let color = Color::new(
                    (base.r as f64 * brightness) as u8,
                    (base.g as f64 * brightness) as u8,
                    (base.b as f64 * brightness) as u8,
                );
                buf.set_color(x, y, color);
            }
        }
    }
}

/// Builder forwarding — so `Glow::on("text").palette(p)` works.
impl On<Glow> {
    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.effect = self.effect.palette(palette);
        self
    }
}

// ── Plasma ──

/// Demoscene plasma: overlapping sine waves create a flowing 2D color field.
pub struct Plasma {
    palette: Vec<Color>,
    seed: f64,
    y_offset: f64,
    total_height: f64,
    total_width: f64,
    palette_ease_frames: usize,
}

impl Plasma {
    pub fn new() -> Self {
        use rand::Rng;
        Self {
            palette: crate::presets::storm().palette(256),
            seed: rand::rng().random::<f64>() * 1000.0,
            y_offset: 0.0,
            total_height: 0.0,
            total_width: 0.0,
            palette_ease_frames: 0,
        }
    }

    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self::new(), text) }

    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.palette = palette;
        self
    }

    pub fn seed(mut self, seed: f64) -> Self {
        self.seed = seed;
        self
    }

    /// Gradually reveal palette colors over N frames.
    /// Starts with only the first color, ends with the full palette.
    pub fn palette_ease(mut self, seconds: f64) -> Self {
        self.palette_ease_frames = super::framebuffer::secs_to_frames(seconds);
        self
    }

    pub fn y_offset(mut self, y_offset: f64) -> Self {
        self.y_offset = y_offset;
        self
    }

    /// Set the total scene dimensions for radial ripple centering.
    /// Without this, each sub-buffer computes its own center.
    pub fn scene_size(mut self, width: f64, height: f64) -> Self {
        self.total_width = width;
        self.total_height = height;
        self
    }
}

impl Effect for Plasma {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 * 0.08;
        let pal = &self.palette;
        if pal.is_empty() { return; }

        let scene_w = if self.total_width > 0.0 { self.total_width } else { buf.width as f64 };
        let scene_h = if self.total_height > 0.0 { self.total_height } else { buf.height as f64 };

        for y in 0..buf.height {
            let yf = y as f64 + self.y_offset;

            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }

                let xf = x as f64;
                let v1 = (xf * 0.08 + t + self.seed).sin();
                let v2 = (yf * 0.12 + t * 0.6 + self.seed * 1.3).sin();
                let v3 = ((xf * 0.06 + yf * 0.08 + t * 0.4 + self.seed * 0.7).sin()
                    + (xf * 0.04 - yf * 0.06 + t * 0.7 + self.seed * 1.9).cos())
                    * 0.5;
                let cx = xf - scene_w / 2.0;
                let cy = (yf - scene_h / 2.0) * 2.5;
                let v4 = ((cx * cx + cy * cy).sqrt() * 0.12 - t * 1.2 + self.seed * 0.5).sin();

                let v = (v1 + v2 + v3 + v4) * 0.25;
                let idx = ((v + 1.0) * 0.5 + t * 0.05).rem_euclid(1.0);
                let max_pal = if self.palette_ease_frames > 0 && frame < self.palette_ease_frames {
                    let progress = frame as f64 / self.palette_ease_frames as f64;
                    let eased = progress * progress;
                    (eased * (pal.len() - 1) as f64).max(0.0)
                } else {
                    (pal.len() - 1) as f64
                };
                let fi = idx * max_pal;
                let lo = (fi.floor() as usize).min(pal.len() - 1);
                let hi = (lo + 1).min(pal.len() - 1);
                let frac = fi - lo as f64;
                let color = Color::lerp_rgb(pal[lo], pal[hi], frac);

                buf.set_color(x, y, color);
            }
        }
    }
}

/// Builder forwarding — so `Plasma::on("text").palette(p).seed(s)` works.
impl On<Plasma> {
    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.effect = self.effect.palette(palette);
        self
    }
    pub fn seed(mut self, seed: f64) -> Self {
        self.effect = self.effect.seed(seed);
        self
    }
    pub fn palette_ease(mut self, seconds: f64) -> Self {
        self.effect = self.effect.palette_ease(seconds);
        self
    }
    pub fn y_offset(mut self, y_offset: f64) -> Self {
        self.effect = self.effect.y_offset(y_offset);
        self
    }
    pub fn scene_size(mut self, width: f64, height: f64) -> Self {
        self.effect = self.effect.scene_size(width, height);
        self
    }
}

// ── Pulse ──

/// Red highlight expanding from center then contracting.
pub struct Pulse;

impl Pulse {
    pub fn new() -> Self { Self }
    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self, text) }
}

impl Effect for Pulse {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let w = buf.content_width();
        let cycle = (frame % 120) + 1;
        let transition = 6;

        let on = Color::new(0xff, 0x10, 0x10);
        let off = Color::new(0xe6, 0xe6, 0xe6);

        let progress = if cycle <= transition {
            cycle as f64 / transition as f64
        } else if cycle <= transition + 10 {
            1.0
        } else {
            let c = cycle - transition - 10;
            1.0 - (c as f64 / transition as f64).min(1.0)
        };
        let half = progress / 2.0;

        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
                let pos = x as f64 / w as f64;
                let dist = (pos - 0.5).abs();
                let color = if dist < half {
                    on
                } else {
                    let t = ((dist - half) / 0.1).min(1.0);
                    Color::lerp_rgb(on, off, t)
                };
                buf.set_color(x, y, color);
            }
        }
    }
}

// ── Glitch ──

/// Random character corruption.
pub struct Glitch {
    chars: Vec<Vec<char>>,
}

impl Glitch {
    pub fn new(text: &str) -> Self {
        Self { chars: text_to_lines(text) }
    }
}

impl Effect for Glitch {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        use rand::Rng;
        let mut rng = rand::rng();
        let glitch_chars = "!@#$%^&*<>[]{}|/\\~`";
        let glitch_vec: Vec<char> = glitch_chars.chars().collect();

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let (out_ch, color) = if rng.random::<f64>() < 0.1 {
                    let g = glitch_vec[rng.random_range(0..glitch_vec.len())];
                    (g, Color::new(
                        rng.random_range(100..=255),
                        rng.random_range(0..=100),
                        rng.random_range(0..=100),
                    ))
                } else {
                    (ch, super::framebuffer::DEFAULT_TEXT_COLOR)
                };
                buf.set(x, y, Cell::new(out_ch, color));
            }
        }
    }

    fn size(&self) -> (usize, usize) { chars_size(&self.chars) }
}

// ── Radar ──

/// Spotlight sweep (angular).
pub struct Radar {
    reverse: bool,
}

impl Radar {
    pub fn new() -> Self { Self { reverse: false } }
    pub fn reversed() -> Self { Self { reverse: true } }
    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self::new(), text) }
}

impl Effect for Radar {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let w = buf.content_width();
        let sweep = if self.reverse {
            1.0 - (frame as f64 * 0.02) % 1.0
        } else {
            (frame as f64 * 0.02) % 1.0
        };

        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
                let pos = x as f64 / w as f64;
                let dist = (pos - sweep).abs().min((pos - sweep + 1.0).abs()).min((pos - sweep - 1.0).abs());
                let brightness = (1.0 - dist * 5.0).max(0.1);
                let color = Color::new(
                    (0x00 as f64 + 0xff as f64 * brightness) as u8,
                    (0xff as f64 * brightness) as u8,
                    (0x00 as f64 + 0x66 as f64 * brightness) as u8,
                );
                buf.set_color(x, y, color);
            }
        }
    }
}

// ── Neon ──

/// Flickering between dim and bright.
pub struct Neon;

impl Neon {
    pub fn new() -> Self { Self }
    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self, text) }
}

impl Effect for Neon {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let bright = Color::new(0xff, 0x44, 0xcc);
        let dim    = Color::new(88, 50, 70);

        // Mostly bright, occasional random flicker to dim
        // Use a simple hash of frame to get pseudo-random per-character flicker
        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }

                // Per-character flicker: hash of (x, y, frame)
                let hash = (x.wrapping_mul(7919) ^ y.wrapping_mul(104729) ^ frame.wrapping_mul(31)) % 100;

                let color = if hash < 6 {
                    // 6% chance of dim flicker per char per frame
                    dim
                } else {
                    // Subtle brightness variation (pulsing glow)
                    let pulse = ((frame as f64 * 0.15 + x as f64 * 0.1).sin() * 0.15 + 0.85).max(0.0);
                    Color::new(
                        (bright.r as f64 * pulse) as u8,
                        (bright.g as f64 * pulse) as u8,
                        (bright.b as f64 * pulse) as u8,
                    )
                };
                buf.set_color(x, y, color);
            }
        }
    }
}

// ── Karaoke ──

/// Progressive character reveal.
pub struct Karaoke;

impl Karaoke {
    pub fn new() -> Self { Self }
    pub fn on(text: &str) -> On<Self> { EffectExt::on(Self, text) }
}

impl Effect for Karaoke {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let highlight = Color::new(0xff, 0xff, 0x00);
        let dim = Color::new(0x66, 0x66, 0x66);

        // Count non-space chars for cycle length
        let total: usize = (0..buf.height)
            .map(|y| (0..buf.width).filter(|&x| buf.get(x, y).ch != ' ').count())
            .sum();
        let revealed = frame % (total + 20);

        let mut count = 0;
        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
                let color = if count < revealed { highlight } else { dim };
                buf.set_color(x, y, color);
                count += 1;
            }
        }
    }
}

// ── Flap ──

/// Split-flap departure board.
pub struct Flap {
    chars: Vec<Vec<char>>,
    settled: Color,
    flipping: Color,
}

impl Flap {
    /// Create a split-flap effect. Default colors: gold settled, dark gold flipping.
    pub fn new(text: &str) -> Self {
        Self {
            chars: text_to_lines(text),
            settled: Color::new(0xff, 0xcc, 0x00),
            flipping: Color::new(0x99, 0x7a, 0x00),
        }
    }

    pub fn settled(mut self, color: Color) -> Self {
        self.settled = color;
        self
    }

    pub fn flipping(mut self, color: Color) -> Self {
        self.flipping = color;
        self
    }
}

impl Effect for Flap {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        use rand::Rng;
        let mut rng = rand::rng();
        let chars_list = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -.:";
        let flap_chars: Vec<char> = chars_list.chars().collect();

        let mut idx = 0;
        for (y, line) in self.chars.iter().enumerate() {
            for (x, &target) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let settle_frame = idx * 2;
                let (ch, color) = if frame >= settle_frame + 10 {
                    (target, self.settled)
                } else if frame >= settle_frame {
                    let f = flap_chars[rng.random_range(0..flap_chars.len())];
                    (f, self.flipping)
                } else {
                    (' ', self.flipping)
                };
                buf.set(x, y, Cell::new(ch, color));
                idx += 1;
            }
        }
    }

    fn size(&self) -> (usize, usize) { chars_size(&self.chars) }
}


// ── Scroll ──

/// Direction from which text slides in.
#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Left,
    Right,
    Top,
    Bottom,
}

/// Slide-in with easing. Text enters from off-screen and settles into place.
pub struct Scroll {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
    direction: ScrollDirection,
    easing: super::easing::Easing,
    total_frames: usize,
    line_delay: usize,
    color_source: Option<Box<dyn Effect>>,
}

impl Scroll {
    /// Create a scroll effect. Defaults: Left, EaseOut, 1 second, no stagger.
    pub fn new(text: &str) -> Self {
        Self {
            chars: text_to_lines(text),
            palette: Vec::new(),
            direction: ScrollDirection::Left,
            easing: super::easing::Easing::EaseOut,
            total_frames: super::framebuffer::secs_to_frames(1.0),
            line_delay: 0,
            color_source: None,
        }
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn easing(mut self, easing: super::easing::Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Duration in seconds.
    pub fn duration(mut self, seconds: f64) -> Self {
        self.total_frames = super::framebuffer::secs_to_frames(seconds);
        self
    }

    /// Per-line stagger in frames. Each successive line starts this many frames later.
    pub fn stagger(mut self, frames: usize) -> Self {
        self.line_delay = frames;
        self
    }

    /// Fallback palette when no `.color()` source is set.
    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.palette = palette;
        self
    }

    /// Color effect applied at rest positions — colors travel with the text.
    pub fn color(mut self, effect: impl Effect) -> Self {
        self.color_source = Some(Box::new(effect));
        self
    }
}

impl Effect for Scroll {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let line_count = self.chars.len();
        let max_width = self.chars.iter().map(|l| l.len()).max().unwrap_or(0);
        if max_width == 0 { return; }

        let term_width = buf.width;
        let pal = &self.palette;

        // If we have a color source, render it at the text's REST positions first.
        // This gives us a pre-colored buffer where colors are anchored to text positions.
        let source_colors = self.color_source.as_ref().map(|cs| {
            let mut color_buf = FrameBuffer::new(buf.width, buf.height);
            // Write chars at their final (rest) positions
            for (y, line) in self.chars.iter().enumerate() {
                for (x, &ch) in line.iter().enumerate() {
                    if x < color_buf.width && y < color_buf.height {
                        color_buf.set(x, y, Cell::new(ch, super::framebuffer::DEFAULT_TEXT_COLOR));
                    }
                }
            }
            cs.render(&mut color_buf, frame);
            color_buf
        });

        for (y, _line) in self.chars.iter().enumerate() {
            if y >= buf.height { break; }

            let line_frame = frame.saturating_sub(y * self.line_delay);
            let t = if self.total_frames == 0 {
                1.0
            } else if frame < y * self.line_delay {
                0.0
            } else {
                (line_frame as f64 / self.total_frames as f64).min(1.0)
            };
            let eased = self.easing.apply(t);

            let h_offset = match self.direction {
                ScrollDirection::Left | ScrollDirection::Right => {
                    let sign = if matches!(self.direction, ScrollDirection::Left) { 1.0 } else { -1.0 };
                    (sign * (1.0 - eased) * term_width as f64).round() as i32
                }
                _ => 0,
            };

            let v_offset = match self.direction {
                ScrollDirection::Top | ScrollDirection::Bottom => {
                    let sign = if matches!(self.direction, ScrollDirection::Top) { 1.0 } else { -1.0 };
                    (sign * (1.0 - eased) * line_count as f64).round() as i32
                }
                _ => 0,
            };

            let src_y = y as i32 + v_offset;

            for x in 0..buf.width {
                let src_x = x as i32 + h_offset;
                let in_bounds = src_y >= 0
                    && (src_y as usize) < line_count
                    && src_x >= 0
                    && (src_x as usize) < max_width;

                let ch = if in_bounds {
                    let src_line = &self.chars[src_y as usize];
                    src_line.get(src_x as usize).copied().unwrap_or(' ')
                } else {
                    ' '
                };

                let color = if ch.is_whitespace() {
                    Color::new(0, 0, 0)
                } else if let Some(ref cb) = source_colors {
                    // Color from the source buffer at the TEXT position (src_x, src_y)
                    if in_bounds {
                        cb.get(src_x as usize, src_y as usize).color
                    } else {
                        super::framebuffer::DEFAULT_TEXT_COLOR
                    }
                } else if !pal.is_empty() {
                    pal[x % pal.len()]
                } else {
                    let hue = (x as f64 / buf.width.max(1) as f64) * 360.0;
                    Color::from_hsv(hue, 0.9, 1.0)
                };

                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }

    fn size(&self) -> (usize, usize) { chars_size(&self.chars) }
}

// ── Fade ──

/// Opacity envelope that wraps another effect.
///
/// Runs the inner effect, then lerps all its colors toward `target_color`
/// based on the current opacity (0.0 = fully target, 1.0 = fully effect).
///
/// Use `Fade::in_from()` for fade-in, `Fade::out_to()` for fade-out.
///
/// **For new code, prefer `Timeline` + `AlphaIn` / `AlphaOut`** — the same
/// effect, but composes with the rest of your animation as separate tracks
/// instead of nesting wrappers.
pub struct Fade {
    inner: Box<dyn Effect>,
    target_color: Color,
    easing: super::easing::Easing,
    total_frames: usize,
    direction: FadeDirection,
}

enum FadeDirection {
    In,  // 0→1 opacity (target → effect)
    Out, // 1→0 opacity (effect → target)
}

impl Fade {
    /// Fade in from `color` over `seconds`.
    pub fn in_from(inner: impl Effect, color: Color, easing: super::easing::Easing, seconds: f64) -> Self {
        Self {
            inner: Box::new(inner),
            target_color: color,
            easing,
            total_frames: super::framebuffer::secs_to_frames(seconds),
            direction: FadeDirection::In,
        }
    }

    /// Fade out to `color` over `seconds`.
    pub fn out_to(inner: impl Effect, color: Color, easing: super::easing::Easing, seconds: f64) -> Self {
        Self {
            inner: Box::new(inner),
            target_color: color,
            easing,
            total_frames: super::framebuffer::secs_to_frames(seconds),
            direction: FadeDirection::Out,
        }
    }
}

impl Effect for Fade {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Run the inner effect first
        self.inner.render(buf, frame);

        // Compute opacity
        let t = if self.total_frames == 0 {
            1.0
        } else {
            (frame as f64 / self.total_frames as f64).min(1.0)
        };
        let eased = self.easing.apply(t);
        let opacity = match self.direction {
            FadeDirection::In => eased,       // 0→1: target→effect
            FadeDirection::Out => 1.0 - eased, // 1→0: effect→target
        };

        // Lerp every cell's color toward target
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch.is_whitespace() { continue; }
                let color = Color::lerp_rgb(self.target_color, cell.color, opacity);
                buf.set_color(x, y, color);
            }
        }
    }

    fn size(&self) -> (usize, usize) { self.inner.size() }
}


/// Chained effect: run A for N seconds, then B, etc.
///
/// **For new code, prefer `Timeline` with `.at(start..end, effect)` calls.**
/// Chain resets the inner effect's frame counter at every stage boundary
/// (so a `Plasma` continuing across stages will visibly stutter), while
/// Timeline lets continuous effects span multiple sub-windows by giving
/// them one long-window track.
pub struct Chain {
    effects: Vec<(usize, Box<dyn Effect>)>, // (duration_frames, effect)
}

impl Chain {
    pub fn new() -> Self {
        Self { effects: Vec::new() }
    }

    /// Add an effect that runs for `seconds`, then the next one starts.
    pub fn then(mut self, seconds: f64, effect: impl Effect) -> Self {
        self.effects.push((super::framebuffer::secs_to_frames(seconds), Box::new(effect)));
        self
    }
}

impl Effect for Chain {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut offset = 0;
        for (duration, effect) in &self.effects {
            if frame < offset + duration {
                effect.render(buf, frame - offset);
                return;
            }
            offset += duration;
        }
        // Past all effects — render the last one at its final frame
        if let Some((duration, effect)) = self.effects.last() {
            effect.render(buf, *duration);
        }
    }

    fn size(&self) -> (usize, usize) {
        self.effects.first().map(|(_, e)| e.size()).unwrap_or((0, 0))
    }
}

// ── Spread ──

/// Lines fan out from a single position to their final rows.
pub struct Spread {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
    origin: SpreadOrigin,
    easing: super::easing::Easing,
    total_frames: usize,
    color_source: Option<Box<dyn Effect>>,
}

/// Where lines start before spreading.
#[derive(Debug, Clone, Copy)]
pub enum SpreadOrigin {
    Top,
    Bottom,
    Center,
}

impl Spread {
    /// Create a spread effect. Defaults: Top origin, EaseOut, 1 second.
    pub fn new(text: &str) -> Self {
        Self {
            chars: text_to_lines(text),
            palette: Vec::new(),
            origin: SpreadOrigin::Top,
            easing: super::easing::Easing::EaseOut,
            total_frames: super::framebuffer::secs_to_frames(1.0),
            color_source: None,
        }
    }

    pub fn origin(mut self, origin: SpreadOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn easing(mut self, easing: super::easing::Easing) -> Self {
        self.easing = easing;
        self
    }

    /// Duration in seconds.
    pub fn duration(mut self, seconds: f64) -> Self {
        self.total_frames = super::framebuffer::secs_to_frames(seconds);
        self
    }

    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.palette = palette;
        self
    }

    pub fn color(mut self, effect: impl Effect) -> Self {
        self.color_source = Some(Box::new(effect));
        self
    }
}

impl Effect for Spread {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let line_count = self.chars.len();
        if line_count == 0 { return; }

        let t = if self.total_frames == 0 {
            1.0
        } else {
            (frame as f64 / self.total_frames as f64).min(1.0)
        };
        let eased = self.easing.apply(t);

        let pal = &self.palette;

        // Pre-color at rest positions if color source exists
        let source_colors = self.color_source.as_ref().map(|cs| {
            let mut color_buf = FrameBuffer::new(buf.width, buf.height);
            for (y, line) in self.chars.iter().enumerate() {
                for (x, &ch) in line.iter().enumerate() {
                    if x < color_buf.width && y < color_buf.height {
                        color_buf.set(x, y, Cell::new(ch, super::framebuffer::DEFAULT_TEXT_COLOR));
                    }
                }
            }
            cs.render(&mut color_buf, frame);
            color_buf
        });

        // Clear buf first — lines will be drawn at computed positions
        for y in 0..buf.height {
            for x in 0..buf.width {
                buf.set(x, y, Cell::space());
            }
        }

        let origin_y = match self.origin {
            SpreadOrigin::Top => 0.0,
            SpreadOrigin::Bottom => (line_count - 1) as f64,
            SpreadOrigin::Center => (line_count - 1) as f64 / 2.0,
        };

        // Draw each line at its interpolated y position
        for (line_idx, line) in self.chars.iter().enumerate() {
            let final_y = line_idx as f64;
            let current_y = origin_y + (final_y - origin_y) * eased;
            let row = current_y.round() as usize;
            if row >= buf.height { continue; }

            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width { continue; }

                let color = if let Some(ref cb) = source_colors {
                    if line_idx < cb.height && x < cb.width {
                        cb.get(x, line_idx).color
                    } else {
                        super::framebuffer::DEFAULT_TEXT_COLOR
                    }
                } else if !pal.is_empty() {
                    pal[x % pal.len()]
                } else {
                    let hue = (x as f64 / buf.width.max(1) as f64) * 360.0;
                    Color::from_hsv(hue, 0.9, 1.0)
                };

                buf.set(x, row, Cell::new(ch, color));
            }
        }
    }

    fn size(&self) -> (usize, usize) { chars_size(&self.chars) }
}

// ── DYCP ──

/// Different Y Character Position — each character bounces on its own sine wave.
///
/// Classic demoscene effect: text ripples vertically like a wave.
/// `amplitude` controls how many rows characters travel (e.g. 3.0 = ±3 rows).
/// `frequency` controls how tight the wave is (higher = more ripples across the width).
/// `speed` controls how fast the wave moves.
pub struct Dycp {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
    amplitude: f64,
    frequency: f64,
    speed: f64,
    scroll_speed: f64,
    scroll_offset: i64,
    ease_in_frames: usize,
    wave_delay: usize,
    phase_offset: f64,
    shadow: Option<(i32, i32, Color)>,
    color_source: Option<Box<dyn Effect>>,
}

impl Dycp {
    /// Create a DYCP effect. Defaults: amplitude 3.0, frequency 0.15, speed 0.08.
    pub fn new(text: &str) -> Self {
        Self {
            chars: text_to_lines(text),
            palette: Vec::new(),
            amplitude: 3.0,
            frequency: 0.15,
            speed: 0.08,
            scroll_speed: 0.0,
            scroll_offset: 0,
            ease_in_frames: 0,
            wave_delay: 0,
            phase_offset: 0.0,
            shadow: None,
            color_source: None,
        }
    }

    pub fn amplitude(mut self, amplitude: f64) -> Self {
        self.amplitude = amplitude;
        self
    }

    pub fn frequency(mut self, frequency: f64) -> Self {
        self.frequency = frequency;
        self
    }

    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    pub fn palette(mut self, palette: Vec<Color>) -> Self {
        self.palette = palette;
        self
    }

    pub fn scroll(mut self, scroll_speed: f64) -> Self {
        self.scroll_speed = scroll_speed;
        self
    }

    /// Start text off-screen and scroll it in. Negative = start right, positive = start left.
    pub fn scroll_in(mut self, offset: i64) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Ease-out the scroll-in over `frames` frames, then rest at natural position.
    /// Must be combined with `scroll_in`. After `frames` the text stays put and only waves.
    pub fn ease_in(mut self, frames: usize) -> Self {
        self.ease_in_frames = frames;
        self
    }

    /// Shift the wave phase (radians). Use to offset a shadow copy.
    pub fn phase_offset(mut self, offset: f64) -> Self {
        self.phase_offset = offset;
        self
    }

    /// Delay the wave by N frames — text scrolls flat, then DYCP kicks in.
    pub fn wave_delay(mut self, frames: usize) -> Self {
        self.wave_delay = frames;
        self
    }

    /// Add a drop shadow at (dx, dy) offset in the given color.
    pub fn shadow(mut self, dx: i32, dy: i32, color: Color) -> Self {
        self.shadow = Some((dx, dy, color));
        self
    }

    pub fn color(mut self, effect: impl Effect) -> Self {
        self.color_source = Some(Box::new(effect));
        self
    }
}

impl Effect for Dycp {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let line_count = self.chars.len();
        if line_count == 0 { return; }
        let pal = &self.palette;
        let t = frame as f64 * self.speed;

        // Pre-color at rest positions
        let source_colors = self.color_source.as_ref().map(|cs| {
            let mut color_buf = FrameBuffer::new(buf.width, buf.height);
            for (y, line) in self.chars.iter().enumerate() {
                for (x, &ch) in line.iter().enumerate() {
                    if x < color_buf.width && y < color_buf.height {
                        color_buf.set(x, y, Cell::new(ch, super::framebuffer::DEFAULT_TEXT_COLOR));
                    }
                }
            }
            cs.render(&mut color_buf, frame);
            color_buf
        });

        // Clear buffer
        for y in 0..buf.height {
            for x in 0..buf.width {
                buf.set(x, y, Cell::space());
            }
        }

        let scroll_px = if self.ease_in_frames > 0 {
            let t = (frame as f64 / self.ease_in_frames as f64).min(1.0);
            let t_ease = 1.0 - (1.0 - t).powi(3); // cubic ease-out
            (self.scroll_offset as f64 * (1.0 - t_ease)) as i64
        } else {
            (frame as f64 * self.scroll_speed) as i64 + self.scroll_offset
        };
        let w = buf.width as i64;
        let wrapping = self.scroll_offset == 0 && self.ease_in_frames == 0;

        for (line_idx, line) in self.chars.iter().enumerate() {
            let base_y = line_idx as f64;

            for (x, &ch) in line.iter().enumerate() {
                if ch.is_whitespace() { continue; }

                // Horizontal scroll
                let screen_x = if wrapping {
                    ((x as i64 - scroll_px) % w + w) % w
                } else {
                    let sx = x as i64 - scroll_px;
                    if sx < 0 || sx >= w { continue; }
                    sx
                };
                let sx = screen_x as usize;

                // Each char gets its own sine offset (0 to amplitude, downward only)
                // Ease-in: linger at top (0), zip through bottom (amplitude)
                // Ramp amplitude from 0 after wave_delay, over 30 frames
                let amp = if self.wave_delay == 0 || frame >= self.wave_delay + 30 {
                    self.amplitude
                } else if frame < self.wave_delay {
                    0.0
                } else {
                    let t_ramp = (frame - self.wave_delay) as f64 / 30.0;
                    self.amplitude * t_ramp * t_ramp
                };

                let wave = (-(x as f64) * self.frequency + t).sin() * 0.6
                    + (-(x as f64) * self.frequency * 2.3 + t * 1.7).sin() * 0.4;
                let normalized = (wave + 1.0) / 2.0;
                let y_offset = normalized * normalized * amp;
                let final_y = (base_y + y_offset).round() as i32;

                if final_y < 0 || final_y as usize >= buf.height { continue; }
                let fy = final_y as usize;

                let color = if let Some(ref cb) = source_colors {
                    if line_idx < cb.height && x < cb.width {
                        cb.get(x, line_idx).color
                    } else {
                        super::framebuffer::DEFAULT_TEXT_COLOR
                    }
                } else if !pal.is_empty() {
                    pal[(x + frame) % pal.len()]
                } else {
                    let hue = (x as f64 / buf.width.max(1) as f64 * 360.0 + t * 10.0) % 360.0;
                    Color::from_hsv(hue, 1.0, 1.0)
                };

                // Shadow pass: draw offset copy in shadow color
                if let Some((dx, dy, shadow_color)) = self.shadow {
                    let shadow_x = sx as i32 + dx;
                    let shadow_y = fy as i32 + dy;
                    if shadow_x >= 0 && (shadow_x as usize) < buf.width
                        && shadow_y >= 0 && (shadow_y as usize) < buf.height
                    {
                        buf.set(shadow_x as usize, shadow_y as usize, Cell::new(ch, shadow_color));
                    }
                }

                buf.set(sx, fy, Cell::new(ch, color));
            }
        }
    }

    fn size(&self) -> (usize, usize) { chars_size(&self.chars) }
}

// ── FadeEnvelope ──

/// Fade in, hold, fade out — smooth opacity envelope over an inner effect.
///
/// **For new code, prefer `Timeline` + `AlphaIn` / `AlphaOut`** as separate
/// tracks. The envelope pattern collapses to:
/// `Timeline.at(0..total, inner).at(0..fade_in, AlphaIn::new(fade_in)).at(total-fade_out..total, AlphaOut::new(fade_out))`.
pub struct FadeEnvelope {
    inner: Box<dyn Effect>,
    target_color: Color,
    fade_out_color: Option<Color>,
    fade_in_frames: usize,
    fade_out_frames: usize,
    total_frames: usize,
    ease_in: super::easing::Easing,
    ease_out: super::easing::Easing,
}

impl FadeEnvelope {
    /// Wrap an effect with a fade envelope. Defaults: 0.5s in, 1s out, EaseOut/EaseInOut, bg color.
    pub fn new(inner: impl Effect) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let default_bg = crate::terminal::bg_color();
        #[cfg(target_arch = "wasm32")]
        let default_bg = Color::new(0, 0, 0);
        Self {
            inner: Box::new(inner),
            target_color: default_bg,
            fade_out_color: None,
            fade_in_frames: super::framebuffer::secs_to_frames(0.5),
            fade_out_frames: super::framebuffer::secs_to_frames(1.0),
            total_frames: super::framebuffer::secs_to_frames(5.0),
            ease_in: super::easing::Easing::EaseOut,
            ease_out: super::easing::Easing::EaseInOut,
        }
    }

    /// Total duration in seconds.
    pub fn total(mut self, seconds: f64) -> Self {
        self.total_frames = super::framebuffer::secs_to_frames(seconds);
        self
    }

    /// Fade-in duration in seconds.
    pub fn fade_in(mut self, seconds: f64, easing: super::easing::Easing) -> Self {
        self.fade_in_frames = super::framebuffer::secs_to_frames(seconds);
        self.ease_in = easing;
        self
    }

    /// Fade-out duration in seconds.
    pub fn fade_out(mut self, seconds: f64, easing: super::easing::Easing) -> Self {
        self.fade_out_frames = super::framebuffer::secs_to_frames(seconds);
        self.ease_out = easing;
        self
    }

    /// Color to fade from/to (default: terminal background).
    pub fn from_color(mut self, color: Color) -> Self {
        self.target_color = color;
        self
    }

    /// Different color to fade out to (default: same as from_color).
    pub fn fade_out_color(mut self, color: Color) -> Self {
        self.fade_out_color = Some(color);
        self
    }
}

impl Effect for FadeEnvelope {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.inner.render(buf, frame);

        let fade_out_start = self.total_frames.saturating_sub(self.fade_out_frames);

        let opacity = if frame < self.fade_in_frames {
            // Fading in
            let t = frame as f64 / self.fade_in_frames.max(1) as f64;
            self.ease_in.apply(t)
        } else if frame >= fade_out_start && self.fade_out_frames > 0 {
            // Fading out
            let t = (frame - fade_out_start) as f64 / self.fade_out_frames as f64;
            1.0 - self.ease_out.apply(t.min(1.0))
        } else {
            1.0 // Fully visible
        };

        if opacity < 1.0 {
            let fade_color = if frame >= fade_out_start && self.fade_out_frames > 0 {
                self.fade_out_color.unwrap_or(self.target_color)
            } else {
                self.target_color
            };
            for y in 0..buf.height {
                for x in 0..buf.width {
                    let cell = buf.get(x, y);
                    if cell.ch.is_whitespace() { continue; }
                    let color = Color::lerp_rgb(fade_color, cell.color, opacity);
                    buf.set_color(x, y, color);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) { self.inner.size() }
}

// ── DelayedStart ──

/// Shows nothing for `delay` frames, then runs the inner effect.
///
/// During the delay, all cells are cleared to spaces. Once the delay
/// is over, the inner effect renders normally with frame counting
/// starting from 0.
///
/// **For new code, prefer `Timeline.at(delay_secs..total, inner)`** —
/// same behaviour, expressed as a track window.
pub struct DelayedStart {
    delay: usize,
    inner: Box<dyn Effect>,
}

impl DelayedStart {
    pub fn new(delay: usize, inner: impl Effect) -> Self {
        Self {
            delay,
            inner: Box::new(inner),
        }
    }
}

impl Effect for DelayedStart {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        if frame < self.delay {
            // Clear our region to spaces
            for y in 0..buf.height {
                for x in 0..buf.width {
                    buf.set(x, y, Cell::space());
                }
            }
        } else {
            self.inner.render(buf, frame - self.delay);
        }
    }

    fn size(&self) -> (usize, usize) { self.inner.size() }
}

// ── Blend ──

/// How two color layers are combined.
#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    /// B replaces A.
    Normal,
    /// A * B / 255 — darker, moody.
    Multiply,
    /// 255 - (255-A)(255-B)/255 — lighter, glowy.
    Screen,
    /// Multiply if dark, Screen if light — contrast boost.
    Overlay,
    /// min(A + B, 255) — blown out, neon.
    Add,
    /// (A + B) / 2 — soft mix.
    Average,
}

impl BlendMode {
    fn apply(self, a: u8, b: u8) -> u8 {
        match self {
            BlendMode::Normal => b,
            BlendMode::Multiply => ((a as u16 * b as u16) / 255) as u8,
            BlendMode::Screen => 255 - (((255 - a as u16) * (255 - b as u16)) / 255) as u8,
            BlendMode::Overlay => {
                if a < 128 {
                    ((2 * a as u16 * b as u16) / 255) as u8
                } else {
                    255 - ((2 * (255 - a as u16) * (255 - b as u16)) / 255) as u8
                }
            }
            BlendMode::Add => (a as u16 + b as u16).min(255) as u8,
            BlendMode::Average => ((a as u16 + b as u16) / 2) as u8,
        }
    }

    fn blend(self, a: Color, b: Color) -> Color {
        Color::new(
            self.apply(a.r, b.r),
            self.apply(a.g, b.g),
            self.apply(a.b, b.b),
        )
    }
}

/// Blend two color effects together.
///
/// Both effects render into separate buffers, then their colors are
/// combined per-cell using the blend mode.
pub struct Blend {
    a: Box<dyn Effect>,
    b: Box<dyn Effect>,
    mode: BlendMode,
}

impl Blend {
    pub fn new(a: impl Effect, b: impl Effect, mode: BlendMode) -> Self {
        Self {
            a: Box::new(a),
            b: Box::new(b),
            mode,
        }
    }
}

impl Effect for Blend {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Render A into buf
        self.a.render(buf, frame);

        // Render B into a scratch buffer
        let mut buf_b = FrameBuffer::new(buf.width, buf.height);
        // Copy chars so B sees the same text layout
        for y in 0..buf.height {
            for x in 0..buf.width {
                buf_b.set(x, y, buf.get(x, y));
            }
        }
        self.b.render(&mut buf_b, frame);

        // Blend colors per cell
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch.is_whitespace() { continue; }
                let color_a = cell.color;
                let color_b = buf_b.get(x, y).color;
                buf.set_color(x, y, self.mode.blend(color_a, color_b));
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        let (aw, ah) = self.a.size();
        let (bw, bh) = self.b.size();
        (aw.max(bw), ah.max(bh))
    }
}

// ── Transition ──

/// Crossfade between two effects over a duration.
///
/// Frame 0: 100% effect A. Frame `duration`: 100% effect B.
/// In between: per-cell color lerp with easing.
pub struct Transition {
    a: Box<dyn Effect>,
    b: Box<dyn Effect>,
    duration: usize,
    easing: super::easing::Easing,
}

impl Transition {
    /// Crossfade from `a` to `b` over `seconds`.
    pub fn new(a: impl Effect, b: impl Effect, seconds: f64, easing: super::easing::Easing) -> Self {
        Self {
            a: Box::new(a),
            b: Box::new(b),
            duration: super::framebuffer::secs_to_frames(seconds),
            easing,
        }
    }
}

impl Effect for Transition {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Render A into buf
        self.a.render(buf, frame);

        if frame >= self.duration {
            // Fully transitioned — just render B
            self.b.render(buf, frame);
            return;
        }

        // Render B into scratch buffer
        let mut buf_b = FrameBuffer::new(buf.width, buf.height);
        for y in 0..buf.height {
            for x in 0..buf.width {
                buf_b.set(x, y, buf.get(x, y));
            }
        }
        self.b.render(&mut buf_b, frame);

        // Lerp colors: t=0 → A, t=1 → B
        let t = self.easing.apply((frame as f64 / self.duration.max(1) as f64).min(1.0));
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch.is_whitespace() { continue; }
                let color_a = cell.color;
                let color_b = buf_b.get(x, y).color;
                let color = Color::lerp_rgb(color_a, color_b, t);
                buf.set_color(x, y, color);
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        let (aw, ah) = self.a.size();
        let (bw, bh) = self.b.size();
        (aw.max(bw), ah.max(bh))
    }
}

// ── Composite ──

/// Combine two effects: one controls character positions, the other controls colors.
///
/// The `position` effect renders first (sets chars + positions).
/// The `color` effect renders into a separate buffer, then its colors
/// are applied to any non-space cells from the position buffer.
pub struct Composite {
    position: Box<dyn Effect>,
    color: Box<dyn Effect>,
}

impl Composite {
    pub fn new(position: impl Effect, color: impl Effect) -> Self {
        Self {
            position: Box::new(position),
            color: Box::new(color),
        }
    }
}

impl Effect for Composite {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Position effect places chars at their current (scrolled) positions
        self.position.render(buf, frame);

        // Color effect renders into a scratch buffer at the same positions
        let mut color_buf = FrameBuffer::new(buf.width, buf.height);
        // Copy chars so the color effect sees text at current positions
        for y in 0..buf.height {
            for x in 0..buf.width {
                color_buf.set(x, y, buf.get(x, y));
            }
        }
        self.color.render(&mut color_buf, frame);

        // Apply colors from the color effect to visible chars
        for y in 0..buf.height {
            for x in 0..buf.width {
                if !buf.get(x, y).ch.is_whitespace() {
                    buf.set_color(x, y, color_buf.get(x, y).color);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        let (aw, ah) = self.position.size();
        let (bw, bh) = self.color.size();
        (aw.max(bw), ah.max(bh))
    }
}

// ── FLD (Flexible Line Distance) ──

/// Flexible Line Distance — shifts entire scanlines vertically with a sine wave.
///
/// Classic demoscene effect: the rendered output ripples like a rubber sheet.
/// Wraps any inner effect, renders it into a scratch buffer, then copies each
/// row to a displaced y position.
pub struct Fld {
    inner: Box<dyn Effect>,
    amplitude: f64,
    frequency: f64,
    speed: f64,
    phase: f64,
    delay: usize,
    ramp: usize,
}

impl Fld {
    /// Wrap an effect with FLD. Defaults: amplitude 3.0, frequency 0.15, speed 0.05.
    pub fn new(inner: impl Effect) -> Self {
        Self {
            inner: Box::new(inner),
            amplitude: 3.0,
            frequency: 0.15,
            speed: 0.05,
            phase: 0.0,
            delay: 0,
            ramp: 30,
        }
    }

    pub fn amplitude(mut self, v: f64) -> Self { self.amplitude = v; self }
    pub fn frequency(mut self, v: f64) -> Self { self.frequency = v; self }
    pub fn speed(mut self, v: f64) -> Self { self.speed = v; self }
    pub fn phase(mut self, v: f64) -> Self { self.phase = v; self }

    /// Frames to wait before bounce starts.
    pub fn delay(mut self, v: usize) -> Self { self.delay = v; self }

    /// Frames to ramp amplitude from 0 to full after delay.
    pub fn ramp(mut self, v: usize) -> Self { self.ramp = v; self }
}

impl Effect for Fld {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Render inner effect into a scratch buffer
        let mut scratch = FrameBuffer::new(buf.width, buf.height);
        self.inner.render(&mut scratch, frame);

        // Before delay: just copy through, no displacement
        if frame < self.delay {
            for y in 0..buf.height {
                for x in 0..buf.width {
                    buf.set(x, y, scratch.get(x, y));
                }
            }
            return;
        }

        // Ramp amplitude from 0 → full over `ramp` frames after delay
        let since = frame - self.delay;
        let amp = if self.ramp > 0 && since < self.ramp {
            let t = since as f64 / self.ramp as f64;
            self.amplitude * t * t // quadratic ease-in
        } else {
            self.amplitude
        };

        buf.clear();

        let t = frame as f64 * self.speed + self.phase;

        // Displace each row
        for y in 0..scratch.height {
            let wave = (y as f64 * self.frequency + t).sin();
            let dy = (wave * amp).round() as i32;
            let dest_y = y as i32 + dy;

            if dest_y < 0 || dest_y as usize >= buf.height { continue; }

            for x in 0..buf.width {
                let cell = scratch.get(x, y);
                if cell.ch != ' ' {
                    buf.set(x, dest_y as usize, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) { self.inner.size() }
}

// ── Wind: cloth-in-wind dissolve ──
//
// One-shot transition. Treats the inner effect's output as a coherent cloth:
// every still-attached cell is displaced by a low-frequency noise-like field
// (sum of sines) so neighbours wave together. A wind force builds over the
// first 70% of the duration. Each cell has a release time τ that grows along
// the wind direction (upwind cells release first). After release, a cell
// becomes a free particle: closed-form damped motion under the wind it saw
// at release, fading out over `fade_tail` seconds.
//
// No per-cell mutable state — the entire effect is a pure function of
// (frame, w, h, params), which keeps the renderer cacheable and the tests
// trivially deterministic.

/// Visual aspect ratio of a terminal cell (rows are roughly twice as tall
/// as columns are wide). Applied to vertical displacement so diagonal wind
/// reads as ~equal angles on screen.
const Y_ASPECT: f64 = 0.5;

pub struct Wind {
    /// `None` = read the existing buffer state as the source (use as a
    /// Timeline track). `Some(_)` = render this effect each frame and
    /// dissolve its output (the wrapper case).
    inner: Option<Box<dyn Effect>>,
    duration: f64,
    delay: f64,
    angle_rad: f64,
    /// Direction the tear front sweeps across the cloth. `None` = same as wind angle
    /// (upwind tears first — the physically natural case). Set explicitly to
    /// decouple visual dissolve order from flight direction.
    tear_angle_rad: Option<f64>,
    strength: f64,
    flutter_amp: f64,
    flutter_freq: f64,
    flutter_speed: f64,
    damping: f64,
    fade_tail: f64,
    /// Pre-release downwind drift: cells lean by `lean × wind_mag` in the wind direction.
    /// Makes the cloth visibly bow into the wind before it tears.
    lean: f64,
    seed: u64,
}

impl Wind {
    /// Read the buffer state as the source — for use as a Timeline track,
    /// or any context where another effect has already populated the buffer.
    /// Builders configure the dissolve: `.duration(...)`, `.angle_deg(...)`, etc.
    pub fn new() -> Self {
        Self::with_inner(None)
    }

    /// Wrap an inner effect — Wind renders the inner each frame into its own
    /// scratch buffer and dissolves the result. Use for "show then blow away"
    /// arcs outside a Timeline.
    pub fn wrap(inner: impl Effect) -> Self {
        Self::with_inner(Some(Box::new(inner)))
    }

    fn with_inner(inner: Option<Box<dyn Effect>>) -> Self {
        Self {
            inner,
            duration: 2.5,
            delay: 0.0,
            angle_rad: 15.0_f64.to_radians(),
            tear_angle_rad: None,
            strength: 30.0,
            flutter_amp: 0.6,
            flutter_freq: 0.25,
            flutter_speed: 2.0,
            damping: 1.5,
            fade_tail: 0.8,
            lean: 0.04,
            seed: 0,
        }
    }

    pub fn duration(mut self, seconds: f64) -> Self { self.duration = seconds; self }
    /// Hold the inner effect statically for `seconds` before any flutter or tearing begins.
    pub fn delay(mut self, seconds: f64) -> Self { self.delay = seconds; self }
    pub fn angle_deg(mut self, deg: f64) -> Self { self.angle_rad = deg.to_radians(); self }
    pub fn angle_rad(mut self, rad: f64) -> Self { self.angle_rad = rad; self }
    /// Direction the tear front sweeps. `0°` = tear starts on the left edge,
    /// `180°` = right edge, `90°` = top, etc. By default the tear follows the wind.
    pub fn tear_angle_deg(mut self, deg: f64) -> Self { self.tear_angle_rad = Some(deg.to_radians()); self }
    pub fn tear_angle_rad(mut self, rad: f64) -> Self { self.tear_angle_rad = Some(rad); self }
    pub fn strength(mut self, cells_per_second: f64) -> Self { self.strength = cells_per_second; self }
    pub fn flutter(mut self, amp: f64, freq: f64, speed: f64) -> Self {
        self.flutter_amp = amp;
        self.flutter_freq = freq;
        self.flutter_speed = speed;
        self
    }
    pub fn damping(mut self, d: f64) -> Self { self.damping = d; self }
    pub fn fade_tail(mut self, seconds: f64) -> Self { self.fade_tail = seconds; self }
    /// Pre-release downwind drift, in cells per (cell/s of wind magnitude).
    /// At default strength=30 and lean=0.04 the cloth bows ~1.2 cells at peak wind.
    pub fn lean(mut self, factor: f64) -> Self { self.lean = factor; self }
    pub fn seed(mut self, s: u64) -> Self { self.seed = s; self }
}

impl Default for Wind {
    fn default() -> Self { Self::new() }
}

#[inline]
fn wind_flutter_offset(c: f64, r: f64, t: f64, amp: f64, freq: f64, speed: f64) -> (f64, f64) {
    let dx = amp * (
        (c * freq + t * speed).sin()
        + ((c + r) * 0.4 * freq + t * speed * 1.3).sin()
    );
    let dy = amp * (
        (r * freq * 1.1 + t * speed * 0.9 + 1.7).sin()
        + ((c - r) * 0.4 * freq + t * speed * 1.1 + 0.3).sin()
    );
    (dx, dy)
}

/// d/dt of `wind_flutter_offset` — used as the initial velocity at release.
#[inline]
fn wind_flutter_velocity(c: f64, r: f64, t: f64, amp: f64, freq: f64, speed: f64) -> (f64, f64) {
    let dvx = amp * speed * (
        (c * freq + t * speed).cos()
        + 1.3 * ((c + r) * 0.4 * freq + t * speed * 1.3).cos()
    );
    let dvy = amp * speed * (
        0.9 * (r * freq * 1.1 + t * speed * 0.9 + 1.7).cos()
        + 1.1 * ((c - r) * 0.4 * freq + t * speed * 1.1 + 0.3).cos()
    );
    (dvx, dvy)
}

/// Cheap deterministic hash → uniform float in [0, 1).
/// Same `(c, r, seed)` always returns the same value — no rand crate calls.
#[inline]
fn wind_hash01(c: usize, r: usize, seed: u64) -> f64 {
    let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= (c as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = h.rotate_left(31);
    h ^= (r as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    h = h.rotate_left(27);
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    ((h >> 11) as f64) / ((1u64 << 53) as f64)
}

#[inline]
fn wind_smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if edge1 <= edge0 { return if x >= edge1 { 1.0 } else { 0.0 }; }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Effect for Wind {
    fn render(&self, out: &mut FrameBuffer, frame: usize) {
        let raw_t = frame as f64 / super::framebuffer::FPS;
        let w = out.width;
        let h = out.height;
        if w == 0 || h == 0 { return; }

        // Hold period: render inner effect straight through, untouched.
        // In buffer mode (no inner), `out` is already populated by earlier
        // tracks — leave it alone.
        if raw_t < self.delay {
            if let Some(inner) = &self.inner {
                inner.render(out, frame);
            }
            return;
        }
        let t = raw_t - self.delay;

        // Build the scratch buffer that holds the source we're going to dissolve.
        let scratch = if let Some(inner) = &self.inner {
            let mut s = FrameBuffer::new(w, h);
            inner.render(&mut s, frame);
            s
        } else {
            // Buffer mode: snapshot the current `out` (populated by earlier
            // Timeline tracks) before we clear it.
            out.clone()
        };
        out.clear();

        let cos_th = self.angle_rad.cos();
        let sin_th = self.angle_rad.sin();
        let tear_angle = self.tear_angle_rad.unwrap_or(self.angle_rad);
        let cos_tr = tear_angle.cos();
        let sin_tr = tear_angle.sin();

        // Normalize the tear projection (c·cos + r·sin) to [0, 1] over the grid,
        // regardless of direction sign. The tear-axis low corner releases first.
        let corners = [
            0.0,
            (w as f64 - 1.0) * cos_tr,
            (h as f64 - 1.0) * sin_tr,
            (w as f64 - 1.0) * cos_tr + (h as f64 - 1.0) * sin_tr,
        ];
        let min_proj = corners.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_proj = corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let proj_range = (max_proj - min_proj).max(1e-9);

        // Per-output-cell highest-α-seen, for collision resolution.
        let mut alpha_grid = vec![-1.0_f64; w * h];
        let bg = Color::new(0, 0, 0);

        for r in 0..h {
            for c in 0..w {
                let cell = scratch.get(c, r);
                if cell.ch == ' ' { continue; }

                let raw_proj = c as f64 * cos_tr + r as f64 * sin_tr;
                let proj = ((raw_proj - min_proj) / proj_range).clamp(0.0, 1.0);
                let release_noise = wind_hash01(c, r, self.seed);
                let tau = self.duration * (0.1 + 0.7 * proj + 0.15 * release_noise);

                let (dx, dy, alpha) = if t < tau {
                    let (mut dx, mut dy) = wind_flutter_offset(
                        c as f64, r as f64, t,
                        self.flutter_amp, self.flutter_freq, self.flutter_speed,
                    );
                    // Pre-release lean: cloth bows into the wind as wind builds.
                    let wm = self.strength
                        * wind_smoothstep(0.0, 0.7 * self.duration, t).powi(2);
                    dx += wm * self.lean * cos_th;
                    dy += wm * self.lean * sin_th;
                    (dx, dy, 1.0)
                } else {
                    let delta = t - tau;
                    if delta >= self.fade_tail { continue; }

                    // Position offset at the moment of release — flutter + lean,
                    // so released cells start from where the leaned attached cell was.
                    let (mut dx0, mut dy0) = wind_flutter_offset(
                        c as f64, r as f64, tau,
                        self.flutter_amp, self.flutter_freq, self.flutter_speed,
                    );
                    let wm_at_tau = self.strength
                        * wind_smoothstep(0.0, 0.7 * self.duration, tau).powi(2);
                    dx0 += wm_at_tau * self.lean * cos_th;
                    dy0 += wm_at_tau * self.lean * sin_th;
                    // Velocity at release (cells/s in flutter terms).
                    let (vfx, vfy) = wind_flutter_velocity(
                        c as f64, r as f64, tau,
                        self.flutter_amp, self.flutter_freq, self.flutter_speed,
                    );
                    // Wind terminal velocity (cells/s) at the moment of release.
                    // Same magnitude as wm_at_tau above — strength is already cells/s
                    // because terminal velocity = wind/k in our model and we treat
                    // strength as terminal velocity directly.
                    let term_vx = wm_at_tau * cos_th;
                    let term_vy = wm_at_tau * sin_th;

                    // Closed-form: dv/dt = -k(v - term_v) →
                    //   v(Δ)        = (v0 - term_v)·exp(-kΔ) + term_v
                    //   pos(Δ) - p0 = (v0 - term_v)/k · (1 - exp(-kΔ)) + term_v·Δ
                    let k = self.damping.max(1e-3);
                    let exp_kt = (-k * delta).exp();
                    let one_minus = 1.0 - exp_kt;
                    let dx_free = (vfx - term_vx) / k * one_minus + term_vx * delta;
                    let dy_free = (vfy - term_vy) / k * one_minus + term_vy * delta;

                    let alpha = (1.0 - delta / self.fade_tail).max(0.0);
                    (dx0 + dx_free, dy0 + dy_free, alpha)
                };

                // Apply terminal aspect ratio to vertical motion only at write time.
                let target_x = (c as f64 + dx).round() as i32;
                let target_y = (r as f64 + dy * Y_ASPECT).round() as i32;
                if target_x < 0 || target_x >= w as i32 { continue; }
                if target_y < 0 || target_y >= h as i32 { continue; }
                let tx = target_x as usize;
                let ty = target_y as usize;

                let idx = ty * w + tx;
                if alpha > alpha_grid[idx] {
                    alpha_grid[idx] = alpha;
                    let dimmed = if alpha >= 0.999 {
                        cell.color
                    } else {
                        Color::lerp_rgb(cell.color, bg, 1.0 - alpha)
                    };
                    out.set(tx, ty, Cell { ch: cell.ch, color: dimmed, bg: cell.bg });
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        match &self.inner {
            Some(inner) => inner.size(),
            None => (0, 0), // buffer mode — caller (Timeline) provides size
        }
    }
}

// ── Timeline: keyframed multi-track stacking ──
//
// `Timeline` lets you compose effects as parallel tracks, each with its own
// activation window. Tracks render in the order they were added; each gets a
// LOCAL frame counter that resets to 0 at the start of its window. Outside
// its window a track is inert.
//
// Example:
// ```ignore
// Timeline::new("hello", 14.0)
//     .at(0.0..2.0, Scroll::new("hello").direction(ScrollDirection::Left))
//     .at(0.0..14.0, Plasma::new().palette(fire))
//     .at(0.0..3.0, AlphaIn::new(3.0))
//     .at(12.0..14.0, AlphaOut::new(2.0))
// ```
//
// Conventions:
// - The `text` argument seeds the buffer with chars (with DEFAULT_TEXT_COLOR)
//   on every frame, so tracks always see the text underneath them. Pass an
//   empty string if you want effects that draw their own chars (Scroll, etc.).
// - For continuous effects you want to "tick smoothly across" multiple
//   sub-windows (e.g., Plasma that fades in then holds), give them ONE
//   long-window track rather than chaining short windows — chained windows
//   reset the inner effect's frame counter and produce visible stutter.

use crate::color::Color as TimelineColor; // alias to keep the use line obvious

pub struct Timeline {
    /// The seed buffer is copied into the render target at the start of each
    /// frame. Built from text via `new`, or supplied directly via `from_buffer`.
    seed: FrameBuffer,
    tracks: Vec<TimelineTrack>,
}

struct TimelineTrack {
    start: usize,
    end: usize,
    effect: Box<dyn Effect>,
}

impl Timeline {
    /// Create a timeline whose base content is `text` (rendered with the
    /// default text colour). Tracks layer on top.
    pub fn new(text: &str, _total_seconds: f64) -> Self {
        Self::from_buffer(
            FrameBuffer::from_text(text, super::framebuffer::DEFAULT_TEXT_COLOR),
            _total_seconds,
        )
    }

    /// Create a timeline from a pre-rendered framebuffer. Useful when the
    /// "base" is something more elaborate than a string — a coloured banner,
    /// a snapshot from another effect, an imported image, etc.
    pub fn from_buffer(seed: FrameBuffer, _total_seconds: f64) -> Self {
        Self { seed, tracks: Vec::new() }
    }

    /// Add a track that activates during `window` (in seconds). The track's
    /// effect receives frame counts starting at 0 when the window opens.
    pub fn at(mut self, window: std::ops::Range<f64>, effect: impl Effect) -> Self {
        self.tracks.push(TimelineTrack {
            start: super::framebuffer::secs_to_frames(window.start),
            end: super::framebuffer::secs_to_frames(window.end),
            effect: Box::new(effect),
        });
        self
    }
}

impl Effect for Timeline {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // Seed the base content every frame.
        let h = self.seed.height.min(buf.height);
        let w = self.seed.width.min(buf.width);
        for y in 0..h {
            for x in 0..w {
                buf.set(x, y, self.seed.get(x, y));
            }
        }

        for track in &self.tracks {
            if frame >= track.start && frame < track.end {
                let local = frame - track.start;
                track.effect.render(buf, local);
            }
        }
    }

    fn size(&self) -> (usize, usize) { (self.seed.width, self.seed.height) }
}

// ── AlphaIn / AlphaOut: buffer-modifying alpha tracks ──
//
// Unlike `Fade` / `FadeEnvelope` these don't wrap an inner effect — they read
// the buffer's current colours and lerp them toward / away from a target
// colour over `seconds`. Designed to be used as Timeline tracks where the
// "content" is provided by other tracks running underneath.

pub struct AlphaIn {
    duration: usize,
    from: TimelineColor,
    easing: super::easing::Easing,
}

impl AlphaIn {
    /// Fade everything from `from_color` (default: black) toward whatever
    /// each cell currently holds, over `seconds`.
    pub fn new(seconds: f64) -> Self {
        Self {
            duration: super::framebuffer::secs_to_frames(seconds).max(1),
            from: TimelineColor::new(0, 0, 0),
            easing: super::easing::Easing::EaseOut,
        }
    }
    pub fn from_color(mut self, c: TimelineColor) -> Self { self.from = c; self }
    pub fn easing(mut self, e: super::easing::Easing) -> Self { self.easing = e; self }
}

impl Effect for AlphaIn {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = self.easing.apply((frame as f64 / self.duration as f64).min(1.0));
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch == ' ' { continue; }
                buf.set_color(x, y, TimelineColor::lerp_rgb(self.from, cell.color, t));
            }
        }
    }
}

pub struct AlphaOut {
    duration: usize,
    to: TimelineColor,
    easing: super::easing::Easing,
}

impl AlphaOut {
    /// Fade everything from each cell's current colour toward `to_color`
    /// (default: black), over `seconds`.
    pub fn new(seconds: f64) -> Self {
        Self {
            duration: super::framebuffer::secs_to_frames(seconds).max(1),
            to: TimelineColor::new(0, 0, 0),
            easing: super::easing::Easing::EaseInOut,
        }
    }
    pub fn to_color(mut self, c: TimelineColor) -> Self { self.to = c; self }
    pub fn easing(mut self, e: super::easing::Easing) -> Self { self.easing = e; self }
}

impl Effect for AlphaOut {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = self.easing.apply((frame as f64 / self.duration as f64).min(1.0));
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch == ' ' { continue; }
                buf.set_color(x, y, TimelineColor::lerp_rgb(cell.color, self.to, t));
            }
        }
    }
}

// Layout effects carry text — give them the same convenience as On<E>.
impl_text_effect_convenience!(Glitch);
impl_text_effect_convenience!(Flap);
impl_text_effect_convenience!(Scroll);
impl_text_effect_convenience!(Spread);
impl_text_effect_convenience!(Dycp);

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buf(text: &str) -> FrameBuffer {
        FrameBuffer::from_text(text, Color::new(255, 255, 255))
    }

    #[test]
    fn rainbow_changes_colors() {
        let effect = Rainbow::new();
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(0, 0).color;
        buf = make_buf("hello");
        effect.render(&mut buf, 10);
        let c1 = buf.get(0, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn rainbow_preserves_chars() {
        let effect = Rainbow::new();
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(4, 0).ch, 'o');
    }

    #[test]
    fn glow_changes_over_time() {
        let pal = vec![Color::new(255, 0, 0), Color::new(0, 0, 255)];
        let effect = Glow::new().palette(pal);
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(2, 0).color;
        buf = make_buf("hello");
        effect.render(&mut buf, 30);
        let c1 = buf.get(2, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn plasma_multiline() {
        let pal = vec![Color::new(255, 0, 0), Color::new(0, 0, 255)];
        let effect = Plasma::new().palette(pal);
        let mut buf = make_buf("ab\ncd");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 1).ch, 'c');
    }

    #[test]
    fn pulse_preserves_chars() {
        let effect = Pulse::new();
        let mut buf = make_buf("test");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 't');
    }

    #[test]
    fn glitch_preserves_length() {
        let effect = Glitch::new("hello\nworld");
        let mut buf = make_buf("hello\nworld");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(4, 0).ch != '\0', true);
        assert_eq!(buf.height, 2);
    }

    #[test]
    fn neon_alternates() {
        let effect = Neon::new();
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        let c0 = buf.get(0, 0).color;
        buf = make_buf("hi");
        effect.render(&mut buf, 1);
        let c1 = buf.get(0, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn karaoke_progressive() {
        let effect = Karaoke::new();
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let dim = buf.get(4, 0).color;
        buf = make_buf("hello");
        effect.render(&mut buf, 10);
        let bright = buf.get(4, 0).color;
        assert_ne!(dim, bright);
    }

    #[test]
    fn flap_settles() {
        let effect = Flap::new("AB");
        let mut buf = make_buf("AB");
        effect.render(&mut buf, 100);
        assert_eq!(buf.get(0, 0).ch, 'A');
        assert_eq!(buf.get(0, 0).color, Color::new(0xff, 0xcc, 0x00)); // default settled
    }

    #[test]
    fn scroll_left_frame_zero_is_blank() {
        let effect = Scroll::new("hello")
            .direction(ScrollDirection::Left)
            .easing(super::super::Easing::BounceOut)
            .duration(2.0);
        let mut buf = FrameBuffer::new(5, 1);
        effect.render(&mut buf, 0);
        for x in 0..5 {
            assert_eq!(buf.get(x, 0).ch, ' ');
        }
    }

    #[test]
    fn scroll_left_final_shows_text() {
        let effect = Scroll::new("hello")
            .direction(ScrollDirection::Left)
            .easing(super::super::Easing::BounceOut)
            .duration(2.0);
        let mut buf = FrameBuffer::new(5, 1);
        effect.render(&mut buf, 60); // 2 seconds * 30fps = 60 frames
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(4, 0).ch, 'o');
    }

    #[test]
    fn scroll_stagger() {
        let effect = Scroll::new("ab\ncd")
            .direction(ScrollDirection::Left)
            .easing(super::super::Easing::Linear)
            .duration(10.0 / 30.0) // 10 frames
            .stagger(5);
        let mut buf = FrameBuffer::new(2, 2);
        effect.render(&mut buf, 5);
    }

    #[test]
    fn fade_in_starts_from_color() {
        let bg = Color::new(0, 0, 0);
        let effect = Fade::in_from(Rainbow::new(), bg, super::super::Easing::Linear, 2.0);
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).color, bg);
    }

    #[test]
    fn fade_in_ends_at_effect_color() {
        let bg = Color::new(0, 0, 0);
        let effect = Fade::in_from(Neon::new(), bg, super::super::Easing::Linear, 2.0);
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 60); // 2 seconds * 30fps
        assert_ne!(buf.get(0, 0).color, bg);
    }

    #[test]
    fn fade_out_ends_at_color() {
        let to = Color::new(0, 0, 0);
        let effect = Fade::out_to(Rainbow::new(), to, super::super::Easing::Linear, 2.0);
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 60);
        assert_eq!(buf.get(0, 0).color, to);
    }

    #[test]
    fn chain_switches_effects() {
        let effect = Chain::new()
            .then(10.0 / 30.0, Rainbow::new())
            .then(10.0 / 30.0, Neon::new());
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        let c_rainbow = buf.get(0, 0).color;
        buf = make_buf("hi");
        effect.render(&mut buf, 15);
        let c_neon = buf.get(0, 0).color;
        assert_ne!(c_rainbow, c_neon);
    }

    #[test]
    fn chain_holds_last_effect() {
        let effect = Chain::new()
            .then(10.0 / 30.0, Fade::in_from(Rainbow::new(), Color::new(0, 0, 0), super::super::Easing::Linear, 10.0 / 30.0));
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 100);
        assert_ne!(buf.get(0, 0).color, Color::new(0, 0, 0));
    }

    fn count_non_space(buf: &FrameBuffer) -> usize {
        let mut n = 0;
        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch != ' ' { n += 1; }
            }
        }
        n
    }

    #[test]
    fn wind_starts_with_inner_text() {
        // At t=0, every character should still be present (some may be displaced
        // by ≤1 cell from flutter, but no fading or release has happened yet).
        let inner = Solid(Color::new(255, 255, 255)).on("hello world");
        let (w, h) = inner.size();
        let effect = Wind::wrap(inner).duration(2.0);
        let mut buf = FrameBuffer::new(w + 2, h + 2);
        <Wind as Effect>::render(&effect, &mut buf, 0);
        let n = count_non_space(&buf);
        // Allow a tiny loss from collisions when two cells round to the same target.
        let expected = "hello world".chars().filter(|c| *c != ' ').count();
        assert!(n >= expected.saturating_sub(2), "want ≥{}, got {}", expected.saturating_sub(2), n);
    }

    #[test]
    fn wind_ends_empty() {
        // Past duration + fade_tail, every cell must be released and faded out.
        let inner = Solid(Color::new(255, 255, 255));
        let effect = Wind::wrap(inner.on("hello world"))
            .duration(1.0)
            .fade_tail(0.3);
        let mut buf = FrameBuffer::new(20, 3);
        // duration (1.0s) + fade_tail (0.3s) + small slack
        let frame = super::super::framebuffer::secs_to_frames(1.0 + 0.3 + 0.2);
        <Wind as Effect>::render(&effect, &mut buf, frame);
        assert_eq!(count_non_space(&buf), 0);
    }

    #[test]
    fn wind_dissolves_over_time() {
        // Stuff goes away, doesn't come back. We can't assert strict monotonicity
        // because flutter + free-flight can cause cells to collide/separate from
        // frame to frame; instead, assert that late counts are strictly below
        // early counts, with margin.
        use super::super::framebuffer::secs_to_frames;
        let inner = Solid(Color::new(255, 255, 255)).on("hello world there");
        let effect = Wind::wrap(inner).duration(1.5);
        let mut buf = FrameBuffer::new(40, 5);

        <Wind as Effect>::render(&effect, &mut buf, 0);
        let n_start = count_non_space(&buf);

        <Wind as Effect>::render(&effect, &mut buf, secs_to_frames(1.5));
        let n_mid = count_non_space(&buf);

        <Wind as Effect>::render(&effect, &mut buf, secs_to_frames(1.5 + 0.4));
        let n_late = count_non_space(&buf);

        assert!(n_mid < n_start, "mid {} should be less than start {}", n_mid, n_start);
        assert!(n_late < n_mid, "late {} should be less than mid {}", n_late, n_mid);
    }

    #[test]
    fn wind_is_deterministic() {
        let make = || Wind::wrap(Solid(Color::new(255, 255, 255)).on("hello")).duration(1.0);
        let mut a = FrameBuffer::new(10, 2);
        let mut b = FrameBuffer::new(10, 2);
        <Wind as Effect>::render(&make(), &mut a, 12);
        <Wind as Effect>::render(&make(), &mut b, 12);
        for y in 0..a.height {
            for x in 0..a.width {
                assert_eq!(a.get(x, y), b.get(x, y), "differ at ({},{})", x, y);
            }
        }
    }

    #[test]
    fn timeline_track_is_inert_outside_window() {
        // Track active 1.0..2.0; at frame 0 it should not affect colors.
        use super::super::framebuffer::secs_to_frames;
        let timeline = Timeline::new("hi", 3.0)
            .at(1.0..2.0, Solid(Color::new(255, 0, 0)));
        let mut buf = FrameBuffer::new(2, 1);
        <Timeline as Effect>::render(&timeline, &mut buf, 0);
        // Default text color, not red.
        assert_ne!(buf.get(0, 0).color, Color::new(255, 0, 0));
        // Inside window: red.
        let mid = secs_to_frames(1.5);
        <Timeline as Effect>::render(&timeline, &mut buf, mid);
        assert_eq!(buf.get(0, 0).color, Color::new(255, 0, 0));
        // After window: red goes away.
        let after = secs_to_frames(2.5);
        <Timeline as Effect>::render(&timeline, &mut buf, after);
        assert_ne!(buf.get(0, 0).color, Color::new(255, 0, 0));
    }

    #[test]
    fn timeline_tracks_layer_in_order() {
        // Two tracks both active; second wins on shared cells.
        let timeline = Timeline::new("x", 2.0)
            .at(0.0..2.0, Solid(Color::new(255, 0, 0)))
            .at(0.0..2.0, Solid(Color::new(0, 255, 0)));
        let mut buf = FrameBuffer::new(1, 1);
        <Timeline as Effect>::render(&timeline, &mut buf, 0);
        assert_eq!(buf.get(0, 0).color, Color::new(0, 255, 0));
    }

    #[test]
    fn alpha_in_lerps_from_black_to_inner() {
        // Solid red track underneath; AlphaIn fades from black to red.
        use super::super::framebuffer::secs_to_frames;
        let timeline = Timeline::new("x", 2.0)
            .at(0.0..2.0, Solid(Color::new(255, 0, 0)))
            .at(0.0..1.0, AlphaIn::new(1.0).easing(super::super::Easing::Linear));
        let mut buf = FrameBuffer::new(1, 1);
        <Timeline as Effect>::render(&timeline, &mut buf, 0);
        // At frame 0 of AlphaIn, color is the from-color (black).
        let c0 = buf.get(0, 0).color;
        assert!(c0.r < 30, "expected near-black, got {:?}", c0);
        // Past AlphaIn duration: full red.
        <Timeline as Effect>::render(&timeline, &mut buf, secs_to_frames(1.5));
        assert_eq!(buf.get(0, 0).color, Color::new(255, 0, 0));
    }

    #[test]
    fn wind_no_panic_across_sizes() {
        use super::super::framebuffer::secs_to_frames;
        for &(w, h) in &[(1, 1), (3, 1), (10, 4), (40, 8), (90, 20)] {
            let inner = Solid(Color::new(255, 255, 255)).on("hello world");
            let effect = Wind::wrap(inner).duration(1.0).fade_tail(0.4);
            let frames_total = secs_to_frames(1.0 + 0.4 + 0.1);
            for f in (0..=frames_total).step_by(5) {
                let mut buf = FrameBuffer::new(w, h);
                <Wind as Effect>::render(&effect, &mut buf, f);
            }
        }
    }
}
