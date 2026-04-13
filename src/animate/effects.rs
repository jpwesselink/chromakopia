//! Native framebuffer effect implementations.
//!
//! Each effect implements the `Effect` trait: takes a `FrameBuffer` and a frame
//! number, writes `(char, Color)` cells directly. No ANSI strings, no parsing.

use crate::color::Color;
use super::framebuffer::{Cell, Effect, EffectExt, On, FrameBuffer, AnimationHandle};

/// Adds `.spawn()`, `.run()`, `.frame()` to effects that carry their own text.
macro_rules! impl_text_effect_convenience {
    ($ty:ty) => {
        impl $ty {
            /// Spawn in a terminal area. Runs until `.stop()` or `.fade_out()`.
            pub fn spawn(self) -> AnimationHandle {
                let (w, h) = <Self as Effect>::size(&self);
                super::framebuffer::spawn_effect(self, w.max(1), h.max(1), 1.0)
            }

            /// Run in a terminal area for `seconds`, then stop.
            pub async fn run(self, seconds: f64) {
                let (w, h) = <Self as Effect>::size(&self);
                super::framebuffer::run_effect(
                    self, w.max(1), h.max(1),
                    std::time::Duration::from_secs_f64(seconds), 1.0,
                ).await;
            }

            /// Render a single frame to an ANSI string.
            pub fn frame(&self, frame: usize) -> String {
                let (w, h) = <Self as Effect>::size(self);
                let mut buf = FrameBuffer::new(w.max(1), h.max(1));
                <Self as Effect>::render(self, &mut buf, frame);
                buf.to_ansi_string()
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
        let color = if frame.is_multiple_of(2) {
            Color::new(88, 80, 85)
        } else {
            Color::new(0xff, 0x44, 0xcc)
        };

        for y in 0..buf.height {
            for x in 0..buf.width {
                if buf.get(x, y).ch == ' ' { continue; }
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

        let term_width = crate::terminal::terminal_width();
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

        let scroll_px = (frame as f64 * self.scroll_speed) as i64 + self.scroll_offset;
        let w = buf.width as i64;
        let wrapping = self.scroll_offset == 0;

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
        Self {
            inner: Box::new(inner),
            target_color: crate::terminal::bg_color(),
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
}
