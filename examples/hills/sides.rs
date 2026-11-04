//! Side scrollers: short extra lines of text that `{side <name>}` in the
//! scroll text sends across the screen once, each at its own height,
//! speed and depth — several at a time, in front of or behind each other,
//! or behind the hills. Each letter rides a DYCP sine of its own (a
//! Different Y for each Character Position), C64 style.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{c64, LETTER_EXTRUDE, LETTER_HIGHLIGHT, LETTER_LOWER, LETTER_OUTLINE, LETTER_UPPER};
use super::tiny;

/// Where things sit back to front: a layer with a higher `z` is painted
/// over one with a lower `z`. Side scrollers pick any `z` in between.
pub mod z {
    pub const SKY: i32 = 0;
    pub const CLOUDS: i32 = 10;
    /// Ridge `i` (0 and 1 are the cloud bands, 7 the ground in front).
    pub const fn ridge(i: usize) -> i32 {
        20 + 10 * i as i32
    }
    pub const SHADOWS: i32 = 100;
    pub const LOGO: i32 = 110;
    pub const PANELS: i32 = 120;
    pub const SCROLLER: i32 = 130;
}

/// The letters a side scroller is set in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Font {
    /// The C64's 8×8.
    C64,
    /// The C64's, twice the size.
    Big,
    /// Tom Thumb's 3×5, as in the captions.
    Tiny,
}

impl Font {
    /// Whether it has `ch`.
    #[cfg(test)]
    fn has(self, ch: char) -> bool {
        match self {
            Font::C64 | Font::Big => super::glyphs::has_c64(ch),
            Font::Tiny => tiny::has(ch),
        }
    }

    /// Pixels from `ch` to the next letter.
    fn advance(self, ch: char) -> i64 {
        match self {
            Font::C64 => 8,
            Font::Big => 16,
            Font::Tiny => tiny::advance(ch),
        }
    }

    /// Pixel rows of a letter, without outline.
    #[cfg(test)]
    pub fn height(self) -> i64 {
        match self {
            Font::C64 => 8,
            Font::Big => 16,
            Font::Tiny => tiny::HEIGHT,
        }
    }

    /// `ch`'s face pixels with its top-left at (x, y), in the letter
    /// colours: cream along the top edges of the C64 letters, pink upper
    /// half, lavender lower.
    fn face(self, ch: char, (x, y): (i64, i64), face: &mut Vec<(i64, i64, Color)>) {
        match self {
            Font::C64 | Font::Big => {
                let scale = if self == Font::Big { 2 } else { 1 };
                let rows = c64(ch);
                let lit = |gx: i64, gy: i64| (0..8).contains(&gy) && rows[gy as usize] & (0x80 >> gx) != 0;
                for gy in 0..8 {
                    for gx in (0..8).filter(|&gx| lit(gx, gy)) {
                        for dy in 0..scale {
                            let color = if dy == 0 && !lit(gx, gy - 1) {
                                LETTER_HIGHLIGHT
                            } else if gy < 4 {
                                LETTER_UPPER
                            } else {
                                LETTER_LOWER
                            };
                            for dx in 0..scale {
                                face.push((x + gx * scale + dx, y + gy * scale + dy, color));
                            }
                        }
                    }
                }
            }
            Font::Tiny => {
                for (gy, row) in tiny::rows(ch).iter().enumerate() {
                    for gx in (0..8).filter(|gx| row & (0x80 >> gx) != 0) {
                        let color = if gy < 3 { LETTER_UPPER } else { LETTER_LOWER };
                        face.push((x + gx, y + gy as i64, color));
                    }
                }
            }
        }
    }
}

/// One side scroller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Side {
    /// What `{side <name>}` calls it.
    pub name: &'static str,
    pub text: &'static str,
    /// Top pixel row of its letters at rest.
    pub y: i64,
    pub font: Font,
    /// Every colour (outline too) moves this far toward this colour: 0
    /// is the main scroller's look.
    pub tone: (Color, f64),
    /// How much it covers what is behind it: 1 solid.
    pub opacity: f64,
    /// Pixels a frame, right to left as the main scroller; negative goes
    /// left to right.
    pub speed: f64,
    /// Depth: see [`z`].
    pub z: i32,
    /// DYCP: pixels up and down each letter swings...
    pub amp: f64,
    /// ...the angle between one letter and the next (radians)...
    pub spread: f64,
    /// ...and seconds for one swing.
    pub period: f64,
}

/// Every side scroller the script can call on.
pub const SIDES: [Side; 4] = [
    // In front of the third hill, ducking in and out behind the fourth.
    Side {
        name: "hide",
        text: "psst... down here!   behind the hills!   hello? can anybody see me?",
        y: 62,
        font: Font::Tiny,
        tone: (LETTER_OUTLINE, 0.35),
        opacity: 1.0,
        speed: 1.0,
        z: z::ridge(4) - 5,
        amp: 3.0,
        spread: 0.15,
        period: 2.0,
    },
    // Way back in the sky, behind the little clouds, slow.
    Side {
        name: "far",
        text: "far, far away... a scroller behind the clouds",
        y: 14,
        font: Font::C64,
        tone: (CLOUD, 0.88),
        opacity: 0.8,
        speed: 0.5,
        z: z::CLOUDS - 5,
        amp: 3.0,
        spread: 0.3,
        period: 2.0,
    },
    // Big and fast, in front of everything, the wrong way round.
    Side {
        name: "rush",
        text: "WHOOSH!",
        y: 30,
        font: Font::Big,
        tone: (CLOUD, 0.0),
        opacity: 1.0,
        speed: -3.0,
        z: z::SCROLLER + 10,
        amp: 10.0,
        spread: 0.7,
        period: 0.9,
    },
    // Over the hills, under the logo.
    Side {
        name: "dycp",
        text: "a different y for every character position - remember DYCP?",
        y: 44,
        font: Font::C64,
        tone: (CLOUD, 0.0),
        opacity: 1.0,
        speed: 2.0,
        z: z::SHADOWS + 5,
        amp: 8.0,
        spread: 0.5,
        period: 1.1,
    },
];

/// The bluish white of a letter that is all cloud.
pub const CLOUD: Color = Color { r: 0xEC, g: 0xF5, b: 0xFF };

/// Pixels a letter's outline and extrusion reach past it: one left, two
/// right.
const EDGE: i64 = 3;

/// The side scroller `{side <name>}` names, as an index into [`SIDES`].
pub fn named(name: &str) -> Option<usize> {
    SIDES.iter().position(|s| s.name == name)
}

impl Side {
    /// Width of the whole text in pixels, outline and extrusion included.
    pub fn length(&self) -> i64 {
        self.text.chars().map(|ch| self.font.advance(ch)).sum::<i64>() + EDGE
    }

    /// Left edge of the text `secs` after its code on a screen `width`
    /// wide: it comes in from the edge it scrolls away from, just out of
    /// sight.
    fn left(&self, secs: f64, width: usize) -> i64 {
        let moved = (secs * FPS * self.speed.abs()).round() as i64;
        if self.speed >= 0.0 {
            width as i64 + 1 - moved
        } else {
            moved - self.length()
        }
    }

    /// Seconds until it has crossed a screen `width` wide and is gone.
    pub fn secs(&self, width: usize) -> f64 {
        (width as i64 + 1 + self.length()) as f64 / (FPS * self.speed.abs())
    }

    /// Whether it is on its way across, `secs` after its code.
    pub fn crossing(&self, secs: f64, width: usize) -> bool {
        (0.0..self.secs(width)).contains(&secs)
    }

    /// Paint it `secs` after its code. Letters off screen are skipped.
    pub fn paint(&self, c: &mut Canvas, secs: f64) {
        if !self.crossing(secs, c.width) {
            return;
        }
        let swing = TAU * secs / self.period;
        let mut face = Vec::new();
        let mut x = self.left(secs, c.width);
        for (i, ch) in self.text.chars().enumerate() {
            let advance = self.font.advance(ch);
            if ch != ' ' && x + advance + 2 >= 0 && x - 2 <= c.width as i64 {
                let dy = (self.amp * (swing + i as f64 * self.spread).sin()).round() as i64;
                self.font.face(ch, (x, self.y + dy), &mut face);
            }
            x += advance;
        }
        // One step of extrusion down-right, the outline, then the faces —
        // all toned.
        let sunk = |color: Color| Color::lerp_rgb(color, self.tone.0, self.tone.1);
        for (shift, color) in [(1, sunk(LETTER_EXTRUDE)), (0, sunk(LETTER_OUTLINE))] {
            for &(x, y, _) in &face {
                for ny in -1..=1 {
                    for nx in -1..=1 {
                        c.set(x + nx + shift, y + ny + shift, color);
                    }
                }
            }
        }
        for &(x, y, color) in &face {
            c.set(x, y, sunk(color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ROWS;
    use super::*;

    #[test]
    fn every_letter_is_in_its_font_and_names_are_unique() {
        for (i, s) in SIDES.iter().enumerate() {
            for ch in s.text.chars() {
                assert!(s.font.has(ch), "{}: no {ch:?}", s.name);
            }
            assert!((0.0..=1.0).contains(&s.tone.1) && (0.0..=1.0).contains(&s.opacity), "{}", s.name);
            assert_eq!(named(s.name), Some(i));
            assert!(s.speed != 0.0, "{} never moves", s.name);
        }
        assert_eq!(named("nope"), None);
    }

    #[test]
    fn swings_stay_on_screen() {
        for s in SIDES {
            let top = s.y as f64 - s.amp - 1.0;
            let bottom = s.y as f64 + s.amp + s.font.height() as f64 + 2.0;
            assert!(top >= 2.0 && bottom <= (ROWS * 2) as f64 - 2.0, "{}: {top}..{bottom}", s.name);
        }
    }

    /// Columns with ink, `secs` after the code.
    fn ink(s: Side, secs: f64, w: usize) -> Vec<usize> {
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| s.paint(c, secs)));
        let mut buf = FrameBuffer::new(w, ROWS);
        layers.render(&mut buf, 0);
        (0..w).filter(|&x| (0..ROWS).any(|y| buf.get(x, y).ch != ' ')).collect()
    }

    #[test]
    fn comes_in_from_the_edge_and_crosses_once() {
        let w = 120;
        let right_to_left = Side { speed: 1.0, ..SIDES[0] };
        let left_to_right = Side { speed: -2.0, ..SIDES[0] };
        for s in [right_to_left, left_to_right] {
            assert!(ink(s, 0.0, w).is_empty(), "{}: on screen straight away", s.speed);
            let mid = ink(s, s.secs(w) / 2.0, w);
            assert!(!mid.is_empty(), "{}: never shows", s.speed);
            assert!(ink(s, s.secs(w) + 0.1, w).is_empty(), "{}: still there", s.speed);
            assert!(ink(s, -1.0, w).is_empty());
        }
        // Early on, each is near the edge it came in from.
        let early = |s: Side| ink(s, 0.5, w);
        assert!(early(right_to_left).iter().all(|&x| x > w / 2));
        assert!(early(left_to_right).iter().all(|&x| x < w / 2));
    }

    #[test]
    fn steps_whole_pixels_at_its_speed() {
        let s = Side { speed: 2.0, ..SIDES[0] };
        let at = |f: usize| s.left(f as f64 / FPS, 100);
        assert_eq!(at(0) - at(1), 2);
        assert_eq!(at(10) - at(13), 6);
    }

    /// Ink cells and their mean brightness, half-way across.
    fn look(s: Side) -> (usize, f64) {
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| s.paint(c, s.secs(c.width) / 2.0)));
        let mut buf = FrameBuffer::new(120, ROWS);
        layers.render(&mut buf, 0);
        let cells: Vec<Color> = (0..ROWS).flat_map(|y| (0..120).map(move |x| (x, y))).map(|(x, y)| buf.get(x, y)).filter(|c| c.ch != ' ').map(|c| c.color).collect();
        let light = cells.iter().map(|c| c.r as f64 + c.g as f64 + c.b as f64).sum::<f64>() / cells.len() as f64;
        (cells.len(), light)
    }

    #[test]
    fn tiny_is_smaller_and_tone_shifts_the_colours() {
        let base = Side { text: "hello", font: Font::C64, tone: (CLOUD, 0.0), amp: 0.0, ..SIDES[0] };
        let (c64_cells, plain) = look(base);
        let (tiny_cells, _) = look(Side { font: Font::Tiny, ..base });
        let (_, dark) = look(Side { tone: (LETTER_OUTLINE, 0.5), ..base });
        let (_, cloud) = look(Side { tone: (CLOUD, 0.8), ..base });
        assert!(tiny_cells * 10 < c64_cells * 7,"tiny {tiny_cells} vs c64 {c64_cells}");
        assert!(dark < plain * 0.9 && cloud > plain * 1.2, "dark {dark}, plain {plain}, cloud {cloud}");
    }

    #[test]
    fn every_letter_rides_its_own_height() {
        let s = Side { text: "MM", font: Font::C64, amp: 6.0, spread: TAU / 4.0, ..SIDES[0] };
        let rows = |col: usize| {
            let layers = Layers::new(ROWS).layer(Layer::paint(move |c| s.paint(c, s.secs(c.width) / 2.0)));
            let mut buf = FrameBuffer::new(60, ROWS);
            layers.render(&mut buf, 0);
            let left = s.left(s.secs(60) / 2.0, 60) as usize;
            (0..ROWS).find(|&y| (left + col..left + col + 8).any(|x| buf.get(x, y).ch != ' '))
        };
        assert_ne!(rows(0), rows(8), "both letters at the same height");
    }
}
