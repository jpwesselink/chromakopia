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

}
