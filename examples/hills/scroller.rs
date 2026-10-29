//! The sales pitch scroller along the bottom.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{c64, stamp};

// Scroller: riding a gentle sine through screen x — classic cracktro
// bottom scroller. Its speed lives in the script's timeline.
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

    /// Paint the text `scrolled` pixels in (0 = just off the right edge;
    /// the script's timeline decides, pauses included). Only the glyphs on
    /// screen are stamped.
    pub fn paint(&self, c: &mut Canvas, scrolled: f64) {
        let t = c.frame as f64 / FPS;
        let start = c.width as i64 - scrolled.floor() as i64;
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

/// The scroll text, cracktro style: brag about what's on screen, never
/// explain how.
pub const PITCH: &str = "Welly welly welly well...   what do we have here?{gap}{logo}{pause 3}{gap}\
...and yes, before you ask: this is a TERMINAL. No browser, no video, no tricks you can see, just the good old terminal you have known and loved...   which you will now love a little bit more.   \
Count the layers behind me: eight bands of clouds and hills, all swinging to their own beat.   \
Clouds that drift and throw shadows on the land.   \
A logo that will not sit still, bouncing every letter on its own little wave.   \
And all of it at a silky 60 frames per second.   \
Up in the box: plasma, rainbows, neon, glitches, flaps, sines, rows that float, letters that fly, \
fields that flow, and colours mixed like a DJ at 3 am.   \
Hit space and they push each other off the stage, one after the other.   \
How is it done?   Ha!   A magician never tells...   \
...but a crate does:   cargo add chromakopia   \
Greetings fly out to everyone who still believes a terminal can be beautiful, \
to all the coders, pixel pushers and scrollers of the past, and to YOU for reading this far!   \
Keys: space or arrows to skip, q to quit.   Text wraps... now!          ";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_is_all_c64_glyphs() {
        let text = super::super::script::parse(PITCH).expect("pitch parses").text;
        for ch in text.chars() {
            assert!(super::super::glyphs::has_c64(ch), "no C64 glyph for {ch:?}");
        }
        assert!(PITCH.contains("cargo add chromakopia"));
    }
}
