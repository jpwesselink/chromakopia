//! Native framebuffer effect implementations.
//!
//! Each effect implements the `Effect` trait: takes a `FrameBuffer` and a frame
//! number, writes `(char, Color)` cells directly. No ANSI strings, no parsing.

use crate::color::Color;
use super::framebuffer::{Cell, Effect, FrameBuffer};

/// Helper: parse text into a Vec of char-lines.
fn text_to_lines(text: &str) -> Vec<Vec<char>> {
    text.split('\n').map(|l| l.chars().collect()).collect()
}

// ── Rainbow ──

/// Rainbow HSV hue rotation across text.
pub struct Rainbow {
    chars: Vec<Vec<char>>,
}

impl Rainbow {
    pub fn new(text: &str) -> Self {
        Self { chars: text_to_lines(text) }
    }
}

impl Effect for Rainbow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let hue_offset = (frame * 5 % 360) as f64;
        let max_w = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let hue = (hue_offset + (x as f64 / max_w as f64) * 360.0) % 360.0;
                buf.set(x, y, Cell::new(ch, Color::from_hsv(hue, 1.0, 1.0)));
            }
        }
    }
}

// ── Glow ──

/// Sweeping spotlight that travels through a gradient palette.
pub struct Glow {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
}

impl Glow {
    pub fn new(text: &str, palette: Vec<Color>) -> Self {
        Self {
            chars: text_to_lines(text),
            palette,
        }
    }
}

impl Effect for Glow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let max_w = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
        let pal = &self.palette;
        if pal.is_empty() { return; }

        let spotlight = (frame as f64 * 0.02).sin() * 0.5 + 0.5;

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let pos = x as f64 / max_w as f64;
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
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}

// ── Plasma ──

/// Demoscene plasma: overlapping sine waves create a flowing 2D color field.
pub struct Plasma {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
    seed: f64,
    y_offset: f64,
}

impl Plasma {
    pub fn new(text: &str, palette: Vec<Color>, seed: f64) -> Self {
        Self {
            chars: text_to_lines(text),
            palette,
            seed,
            y_offset: 0.0,
        }
    }

    pub fn with_y_offset(mut self, y_offset: f64) -> Self {
        self.y_offset = y_offset;
        self
    }
}

impl Effect for Plasma {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 * 0.08;
        let pal = &self.palette;
        if pal.is_empty() { return; }

        for (y, line) in self.chars.iter().enumerate() {
            if y >= buf.height { break; }
            let yf = y as f64 + self.y_offset;

            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width { break; }

                let xf = x as f64;
                let v1 = (xf * 0.08 + t + self.seed).sin();
                let v2 = (yf * 0.12 + t * 0.6 + self.seed * 1.3).sin();
                let v3 = ((xf * 0.06 + yf * 0.08 + t * 0.4 + self.seed * 0.7).sin()
                    + (xf * 0.04 - yf * 0.06 + t * 0.7 + self.seed * 1.9).cos())
                    * 0.5;
                let cx = xf - buf.width as f64 / 2.0;
                let cy = (yf - buf.height as f64 / 2.0) * 2.5;
                let v4 = ((cx * cx + cy * cy).sqrt() * 0.12 - t * 1.2 + self.seed * 0.5).sin();

                let v = (v1 + v2 + v3 + v4) * 0.25;
                let idx = ((v + 1.0) * 0.5 + t * 0.05).rem_euclid(1.0);
                let fi = idx * (pal.len() - 1) as f64;
                let lo = (fi.floor() as usize).min(pal.len() - 1);
                let hi = (lo + 1).min(pal.len() - 1);
                let frac = fi - lo as f64;
                let color = Color::lerp_rgb(pal[lo], pal[hi], frac);

                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}

// ── Pulse ──

/// Red highlight expanding from center then contracting.
pub struct Pulse {
    chars: Vec<Vec<char>>,
}

impl Pulse {
    pub fn new(text: &str) -> Self {
        Self { chars: text_to_lines(text) }
    }
}

impl Effect for Pulse {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let max_w = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
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

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let pos = x as f64 / max_w as f64;
                let dist = (pos - 0.5).abs();
                let color = if dist < half {
                    on
                } else {
                    let t = ((dist - half) / 0.1).min(1.0);
                    Color::lerp_rgb(on, off, t)
                };
                buf.set(x, y, Cell::new(ch, color));
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
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
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
                    (ch, Color::new(0xcc, 0xcc, 0xcc))
                };
                buf.set(x, y, Cell::new(out_ch, color));
            }
        }
        let _ = frame; // glitch is random per frame, frame unused
    }
}

// ── Radar ──

/// Spotlight sweep (angular).
pub struct Radar {
    chars: Vec<Vec<char>>,
}

impl Radar {
    pub fn new(text: &str) -> Self {
        Self { chars: text_to_lines(text) }
    }
}

impl Effect for Radar {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let max_w = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
        let sweep = (frame as f64 * 0.02) % 1.0;

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let pos = x as f64 / max_w as f64;
                let dist = (pos - sweep).abs().min((pos - sweep + 1.0).abs()).min((pos - sweep - 1.0).abs());
                let brightness = (1.0 - dist * 5.0).max(0.1);
                let color = Color::new(
                    (0x00 as f64 + 0xff as f64 * brightness) as u8,
                    (0xff as f64 * brightness) as u8,
                    (0x00 as f64 + 0x66 as f64 * brightness) as u8,
                );
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}

// ── Neon ──

/// Flickering between dim and bright.
pub struct Neon {
    chars: Vec<Vec<char>>,
}

impl Neon {
    pub fn new(text: &str) -> Self {
        Self { chars: text_to_lines(text) }
    }
}

impl Effect for Neon {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let color = if frame.is_multiple_of(2) {
            Color::new(88, 80, 85) // dim
        } else {
            Color::new(0xff, 0x44, 0xcc) // bright neon pink
        };

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}

// ── Karaoke ──

/// Progressive character reveal.
pub struct Karaoke {
    chars: Vec<Vec<char>>,
    total_chars: usize,
}

impl Karaoke {
    pub fn new(text: &str) -> Self {
        let chars = text_to_lines(text);
        let total_chars = chars.iter().map(|l| l.len()).sum();
        Self { chars, total_chars }
    }
}

impl Effect for Karaoke {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let highlight = Color::new(0xff, 0xff, 0x00);
        let dim = Color::new(0x66, 0x66, 0x66);
        let revealed = frame % (self.total_chars + 20); // cycle with pause

        let mut count = 0;
        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let color = if count < revealed { highlight } else { dim };
                buf.set(x, y, Cell::new(ch, color));
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
    pub fn new(text: &str, settled: Color, flipping: Color) -> Self {
        Self {
            chars: text_to_lines(text),
            settled,
            flipping,
        }
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
///
/// `total_frames` controls how long the slide takes.
/// `line_delay` staggers each line's start for a slant/cascade effect.
pub struct Scroll {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
    direction: ScrollDirection,
    easing: super::easing::Easing,
    total_frames: usize,
    line_delay: usize,
}

impl Scroll {
    pub fn new(
        text: &str,
        palette: Vec<Color>,
        direction: ScrollDirection,
        easing: super::easing::Easing,
        total_frames: usize,
        line_delay: usize,
    ) -> Self {
        Self {
            chars: text_to_lines(text),
            palette,
            direction,
            easing,
            total_frames,
            line_delay,
        }
    }
}

impl Effect for Scroll {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let line_count = self.chars.len();
        let max_width = self.chars.iter().map(|l| l.len()).max().unwrap_or(0);
        if max_width == 0 { return; }

        let term_width = crate::terminal::terminal_width();
        let pal = &self.palette;

        for (y, line) in self.chars.iter().enumerate() {
            if y >= buf.height { break; }

            // Per-line stagger
            let line_frame = frame.saturating_sub(y * self.line_delay);
            let t = if self.total_frames == 0 {
                1.0
            } else if frame < y * self.line_delay {
                0.0
            } else {
                (line_frame as f64 / self.total_frames as f64).min(1.0)
            };
            let eased = self.easing.apply(t);

            // Horizontal offset
            let h_offset = match self.direction {
                ScrollDirection::Left | ScrollDirection::Right => {
                    let sign = if matches!(self.direction, ScrollDirection::Left) { 1.0 } else { -1.0 };
                    if eased <= 1.0 {
                        (sign * (1.0 - eased) * max_width as f64).round() as i32
                    } else {
                        (sign * (1.0 - eased) * term_width as f64).round() as i32
                    }
                }
                _ => 0,
            };

            // Vertical offset
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
                let ch = if src_y >= 0
                    && (src_y as usize) < line_count
                    && src_x >= 0
                    && (src_x as usize) < max_width
                {
                    let src_line = &self.chars[src_y as usize];
                    src_line.get(src_x as usize).copied().unwrap_or(' ')
                } else {
                    ' '
                };

                let color = if ch.is_whitespace() {
                    Color::new(0, 0, 0)
                } else if !pal.is_empty() {
                    let idx = x % pal.len();
                    pal[idx]
                } else {
                    let hue = (x as f64 / buf.width.max(1) as f64) * 360.0;
                    Color::from_hsv(hue, 0.9, 1.0)
                };

                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
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
    /// Fade in: starts at `from_color`, reveals the inner effect over `total_frames`.
    pub fn in_from(inner: impl Effect, from_color: Color, easing: super::easing::Easing, total_frames: usize) -> Self {
        Self {
            inner: Box::new(inner),
            target_color: from_color,
            easing,
            total_frames,
            direction: FadeDirection::In,
        }
    }

    /// Fade out: starts showing the inner effect, fades to `to_color` over `total_frames`.
    pub fn out_to(inner: impl Effect, to_color: Color, easing: super::easing::Easing, total_frames: usize) -> Self {
        Self {
            inner: Box::new(inner),
            target_color: to_color,
            easing,
            total_frames,
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
}

// Keep the old names as convenience constructors
pub type FadeIn = Fade;
pub type FadeOut = Fade;

/// Chained effect: runs effect A for N frames, then effect B.
///
/// Use this to chain fade-in → hold → fade-out, or any sequence.
pub struct Chain {
    effects: Vec<(usize, Box<dyn Effect>)>, // (duration_frames, effect)
}

impl Chain {
    pub fn new() -> Self {
        Self { effects: Vec::new() }
    }

    /// Add an effect that runs for `frames` frames, then the next one starts.
    pub fn then(mut self, frames: usize, effect: impl Effect) -> Self {
        self.effects.push((frames, Box::new(effect)));
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
        // Position effect sets chars
        self.position.render(buf, frame);

        // Color effect renders into a scratch buffer
        let mut color_buf = FrameBuffer::new(buf.width, buf.height);
        self.color.render(&mut color_buf, frame);

        // Apply colors from color_buf to non-space cells in buf
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if !cell.ch.is_whitespace() {
                    buf.set_color(x, y, color_buf.get(x, y).color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buf(text: &str) -> FrameBuffer {
        FrameBuffer::from_text(text, Color::new(255, 255, 255))
    }

    #[test]
    fn rainbow_changes_colors() {
        let effect = Rainbow::new("hello");
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(0, 0).color;
        effect.render(&mut buf, 10);
        let c1 = buf.get(0, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn rainbow_preserves_chars() {
        let effect = Rainbow::new("hello");
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(4, 0).ch, 'o');
    }

    #[test]
    fn glow_changes_over_time() {
        let pal = vec![Color::new(255, 0, 0), Color::new(0, 0, 255)];
        let effect = Glow::new("hello", pal);
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(2, 0).color;
        effect.render(&mut buf, 30);
        let c1 = buf.get(2, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn plasma_multiline() {
        let effect = Plasma::new("ab\ncd", vec![Color::new(255, 0, 0), Color::new(0, 0, 255)], 0.0);
        let mut buf = make_buf("ab\ncd");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 1).ch, 'c');
    }

    #[test]
    fn pulse_preserves_chars() {
        let effect = Pulse::new("test");
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
        let effect = Neon::new("hi");
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        let c0 = buf.get(0, 0).color;
        effect.render(&mut buf, 1);
        let c1 = buf.get(0, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn karaoke_progressive() {
        let effect = Karaoke::new("hello");
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let dim = buf.get(4, 0).color;
        effect.render(&mut buf, 10);
        let bright = buf.get(4, 0).color;
        // After 10 frames all 5 chars should be revealed
        assert_ne!(dim, bright);
    }

    #[test]
    fn flap_settles() {
        let s = Color::new(255, 204, 0);
        let f = Color::new(153, 122, 0);
        let effect = Flap::new("AB", s, f);
        let mut buf = make_buf("AB");
        effect.render(&mut buf, 100);
        assert_eq!(buf.get(0, 0).ch, 'A');
        assert_eq!(buf.get(0, 0).color, s);
    }

    #[test]
    fn scroll_left_frame_zero_is_blank() {
        let pal = vec![Color::new(255, 255, 255)];
        let effect = Scroll::new("hello", pal, ScrollDirection::Left, super::super::Easing::BounceOut, 60, 0);
        let mut buf = FrameBuffer::new(5, 1);
        effect.render(&mut buf, 0);
        // All spaces at frame 0 — text is off-screen
        for x in 0..5 {
            assert_eq!(buf.get(x, 0).ch, ' ');
        }
    }

    #[test]
    fn scroll_left_final_shows_text() {
        let pal = vec![Color::new(255, 255, 255)];
        let effect = Scroll::new("hello", pal, ScrollDirection::Left, super::super::Easing::BounceOut, 60, 0);
        let mut buf = FrameBuffer::new(5, 1);
        effect.render(&mut buf, 60);
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(4, 0).ch, 'o');
    }

    #[test]
    fn scroll_stagger() {
        let pal = vec![Color::new(255, 255, 255)];
        let effect = Scroll::new("ab\ncd", pal, ScrollDirection::Left, super::super::Easing::Linear, 10, 5);
        let mut buf = FrameBuffer::new(2, 2);
        // At frame 5, first line should be halfway, second line hasn't started
        effect.render(&mut buf, 5);
        // Second line should still be blank (delay=5, so it starts at frame 5)
        // At exactly frame 5 for line 1: t=0.5, offset ~1 char
        // Line 1 has started, line 2 just starting (t=0.0)
    }

    #[test]
    fn fade_in_starts_from_color() {
        let bg = Color::new(0, 0, 0);
        let inner = Rainbow::new("hi");
        let effect = Fade::in_from(inner, bg, super::super::Easing::Linear, 60);
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        // At frame 0, opacity=0, so fully bg color
        assert_eq!(buf.get(0, 0).color, bg);
    }

    #[test]
    fn fade_in_ends_at_effect_color() {
        let bg = Color::new(0, 0, 0);
        // Use a simple effect that sets a known color
        let inner = Neon::new("hi"); // frame 1 = bright pink
        let effect = Fade::in_from(inner, bg, super::super::Easing::Linear, 60);
        let mut buf = make_buf("hi");
        // At frame 60, opacity=1, so fully the inner effect's color
        effect.render(&mut buf, 60);
        assert_ne!(buf.get(0, 0).color, bg);
    }

    #[test]
    fn fade_out_ends_at_color() {
        let to = Color::new(0, 0, 0);
        let inner = Rainbow::new("hi");
        let effect = Fade::out_to(inner, to, super::super::Easing::Linear, 60);
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 60);
        // At frame 60, opacity=0, so fully target color
        assert_eq!(buf.get(0, 0).color, to);
    }

    #[test]
    fn chain_switches_effects() {
        let effect = Chain::new()
            .then(10, Rainbow::new("hi"))
            .then(10, Neon::new("hi"));
        let mut buf = make_buf("hi");
        effect.render(&mut buf, 0);
        let c_rainbow = buf.get(0, 0).color;
        effect.render(&mut buf, 15);
        let c_neon = buf.get(0, 0).color;
        assert_ne!(c_rainbow, c_neon);
    }

    #[test]
    fn chain_holds_last_effect() {
        let effect = Chain::new()
            .then(10, Fade::in_from(Rainbow::new("hi"), Color::new(0, 0, 0), super::super::Easing::Linear, 10));
        let mut buf = make_buf("hi");
        // Past the end — should hold final frame (fully revealed rainbow)
        effect.render(&mut buf, 100);
        assert_ne!(buf.get(0, 0).color, Color::new(0, 0, 0)); // not black = faded in
    }
}
