use crate::color::Color;
use super::framebuffer::{Cell, Effect, FrameBuffer};

/// Native framebuffer plasma — writes directly to the grid, no ANSI strings.
pub struct Plasma {
    text: Vec<Vec<char>>,
    palette: Vec<Color>,
    seed: f64,
}

impl Plasma {
    pub fn new(text: &str, palette: Vec<Color>, seed: f64) -> Self {
        let text: Vec<Vec<char>> = text
            .split('\n')
            .map(|line| line.chars().collect())
            .collect();
        Self { text, palette, seed }
    }
}

impl Effect for Plasma {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 * 0.08;
        let pal = &self.palette;
        if pal.is_empty() {
            return;
        }

        for y in 0..buf.height {
            let yf = y as f64;
            let text_line = self.text.get(y);
            let line_width = text_line.map_or(0, |l| l.len());

            for x in 0..buf.width {
                let ch = text_line
                    .and_then(|l| l.get(x).copied())
                    .unwrap_or(' ');

                if ch.is_whitespace() && x >= line_width {
                    buf.set(x, y, Cell::space());
                    continue;
                }

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

/// Native framebuffer sparkle — stars radiate from center.
pub struct Sparkle {
    text: Vec<Vec<char>>,
    palette: Vec<Color>,
}

impl Sparkle {
    pub fn new(text: &str, palette: Vec<Color>) -> Self {
        let text: Vec<Vec<char>> = text
            .split('\n')
            .map(|line| line.chars().collect())
            .collect();
        Self { text, palette }
    }
}

impl Effect for Sparkle {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let cx = buf.width as f64 / 2.0;
        let cy = buf.height as f64 / 2.0;
        let max_dist = (cx * cx + (cy * 2.5) * (cy * 2.5)).sqrt();
        let t = frame as f64;
        let pal = &self.palette;

        for y in 0..buf.height {
            let text_line = self.text.get(y);

            for x in 0..buf.width {
                let ch = text_line
                    .and_then(|l| l.get(x).copied())
                    .unwrap_or(' ');

                if ch.is_whitespace() {
                    buf.set(x, y, Cell::space());
                    continue;
                }

                let dx = x as f64 - cx;
                let dy = (y as f64 - cy) * 2.5;
                let dist = (dx * dx + dy * dy).sqrt() / max_dist;

                let phase = ((x * 3571 + y * 2719) % 997) as f64;
                let speed = 0.2 + dist * 0.8;
                let cycle = ((t * speed * 0.15 + phase) % 40.0) / 40.0;
                let pulse = (cycle * std::f64::consts::TAU).sin() * 0.5 + 0.5;
                let brightness = dist * (0.3 + 0.7 * pulse);

                let color = if pal.is_empty() {
                    let r = (200.0 * brightness + 55.0 * dist) as u8;
                    let g = (220.0 * brightness + 35.0 * dist) as u8;
                    let b = (255.0 * brightness) as u8;
                    Color::new(r, g, b)
                } else {
                    let color_t = (dist + cycle * 0.3).rem_euclid(1.0);
                    let fi = color_t * (pal.len() - 1) as f64;
                    let lo = (fi.floor() as usize).min(pal.len() - 1);
                    let hi = (lo + 1).min(pal.len() - 1);
                    let frac = fi - lo as f64;
                    let base = Color::lerp_rgb(pal[lo], pal[hi], frac);
                    Color::new(
                        (base.r as f64 * brightness) as u8,
                        (base.g as f64 * brightness) as u8,
                        (base.b as f64 * brightness) as u8,
                    )
                };

                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}
