//! The sales pitch scroller along the bottom.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{c64, stamp};

// Scroller: one pixel per frame (smooth at any FPS-locked rate), riding a
// gentle sine through screen x — classic cracktro bottom scroller.
const SCROLL_SPEED: f64 = FPS;
const SCROLL_WAVE: f64 = 4.0;
const SCROLL_WAVE_PERIOD: f64 = 0.8;

/// Text in C64 glyphs, scrolling right to left along the bottom.
pub struct Scroller {
    glyphs: Vec<[u8; 8]>,
    y0: i64,
}

impl Scroller {
    pub fn new(text: &str, y0: i64) -> Self {
        let flat = text.replace("\n\n", "   ").replace('\n', " ");
        let glyphs = flat
            .chars()
            .map(c64)
            .collect();
        Self { glyphs, y0 }
    }

    /// Only the glyphs on screen are stamped. The text enters from the
    /// right edge, runs out, and starts again.
    pub fn paint(&self, c: &mut Canvas) {
        let t = c.frame as f64 / FPS;
        let run = (self.glyphs.len() * 8 + c.width) as i64;
        let scrolled = (t * SCROLL_SPEED) as i64 % run;
        let start = c.width as i64 - scrolled;
        let first = (-start - 8).max(0) / 8;
        let mut face = Vec::new();
        for i in first as usize..self.glyphs.len() {
            let x = start + i as i64 * 8;
            if x > c.width as i64 {
                break;
            }
            let wave = (TAU * t / SCROLL_WAVE_PERIOD + x as f64 * 0.05).sin();
            let y = self.y0 + (SCROLL_WAVE * wave).round() as i64;
            stamp(c, &self.glyphs[i], (x, y), 1, 1, &mut face);
        }
    }
}

/// The pitch, cracktro style: why an engineer would want this crate.
pub const PITCH: &str = "Hey engineers!   Yes, you, staring at that grey spinner while your build crawls...   \
Your CLI deserves better.   CHROMAKOPIA turns your terminal into a demo scene, in Rust.   \
Plasma, rainbow, glow, neon, glitch, flap, marquee, DYCP, FLD, vector fields, height maps:   \
each one a single line of code.   \
Under the hood a real framebuffer at 60 fps with diff rendering, so only the cells that change get redrawn.   \
Half-block pixels for twice the vertical resolution.   \
Stack layers, sway them, blend them: normal, multiply, screen and friends, on any cell, glyph or pixel.   \
This whole show renders in under a millisecond a frame. The other 15 ms are yours.   \
The same code runs in the browser through WASM.   MIT licensed, so ship it tomorrow.   \
cargo add chromakopia   ...and make your users smile.   \
Greetings to everyone who still believes a terminal can be beautiful!   \
Keys: space for the next effect, arrows to skip around, q to quit.   ...wrapping around...          ";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_is_all_c64_glyphs() {
        for ch in PITCH.chars().filter(|&c| c != '\n') {
            assert!(super::super::glyphs::has_c64(ch), "no C64 glyph for {ch:?}");
        }
        assert!(PITCH.contains("cargo add chromakopia"));
    }
}
