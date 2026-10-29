//! The licence scroller along the bottom.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{c64, stamp};

// Scroller: one pixel per frame (smooth at any FPS-locked rate), riding a
// gentle sine through screen x — classic cracktro bottom scroller.
const SCROLL_SPEED: f64 = FPS;
const SCROLL_WAVE: f64 = 4.0;
const SCROLL_WAVE_PERIOD: f64 = 0.8;

/// The licence, in C64 glyphs, scrolling right to left along the bottom.
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

pub const LICENSE_TEXT: &str = r#"MIT License

Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE."#;
