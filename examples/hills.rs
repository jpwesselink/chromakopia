//! Hills — stacked dithered ridge layers swaying left and right.
//!
//! Every layer is a row of rounded humps (|sin| — round tops, sharp
//! valleys) filled with a 2×2 ordered-dither mix of two pastels: sky,
//! clouds, mint hills and peach ground. All layers make the same eased
//! back-and-forth swing, each a little out of phase with the one behind
//! it, so the motion ripples through the stack front to back. A band of
//! cloud shadows drifts across the land as a `Multiply` layer.
//!
//! Last, centred over it all: CHROMAKOPIA in chunky Mario-title-style
//! letters — pastel pink fading to lavender with a cream top edge, deep
//! plum outline and 3D extrusion down-right. The letters drop in one by
//! one from above the screen with a bounce, then each grows into its own
//! DYCP sine. They're painted left to right, each fully before the next,
//! so every letter covers its left neighbour's extrusion.
//!
//! Along the bottom, the MIT licence scrolls by in the C64 character ROM
//! font, same pastel styling, riding a gentle sine — cracktro style.
//!
//! Run: `cargo run --release --example hills`   (press q to quit)

use chromakopia::animate::dither;
use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::{PI, TAU};

// Cloud shadows multiply with NES blue (2C02 entry 22).
const SHADOW_BLUE: Color = Color { r: 0x68, g: 0x88, b: 0xFC };

// Pastel landscape, lightest at the back.
const SKY: Color = Color { r: 0xA8, g: 0xD8, b: 0xFF };
const SKY_HAZE: Color = Color { r: 0xC9, g: 0xC2, b: 0xFF };
const CLOUD_LILAC: Color = Color { r: 0xC8, g: 0xB4, b: 0xFF };
const CLOUD_PINK: Color = Color { r: 0xFF, g: 0xC4, b: 0xDE };
const CLOUD_BLUSH: Color = Color { r: 0xFF, g: 0xE0, b: 0xEE };
const MINT_MIST: Color = Color { r: 0xD4, g: 0xF5, b: 0xD0 };
const MINT_PALE: Color = Color { r: 0xBD, g: 0xED, b: 0xB8 };
const MINT: Color = Color { r: 0xA6, g: 0xE3, b: 0xA6 };
const PISTACHIO: Color = Color { r: 0xD8, g: 0xF0, b: 0xA8 };
const SAGE: Color = Color { r: 0x8C, g: 0xD1, b: 0x9E };
const SAGE_DEEP: Color = Color { r: 0x76, g: 0xBF, b: 0x93 };
const PEACH: Color = Color { r: 0xFF, g: 0xD4, b: 0xB8 };
const CORAL: Color = Color { r: 0xF7, g: 0xB0, b: 0x98 };
const ROSE: Color = Color { r: 0xDE, g: 0x98, b: 0x92 };
const LETTER_CREAM: Color = Color { r: 0xFF, g: 0xF8, b: 0xE7 };
const LETTER_PINK: Color = Color { r: 0xFF, g: 0xB3, b: 0xC7 };
const LETTER_LAVENDER: Color = Color { r: 0xB9, g: 0xA2, b: 0xF0 };
const LETTER_OUTLINE: Color = Color { r: 0x2A, g: 0x16, b: 0x38 };
const LETTER_EXTRUDE: Color = Color { r: 0x4E, g: 0x30, b: 0x6E };

/// Scene height in terminal rows (two pixels each).
const ROWS: usize = 40;
/// Rows of open sky above the landscape, so the wordmark has headroom.
const TOP_PAD: usize = 6;

/// Seconds for one full left→right→left swing.
const PERIOD: f64 = 4.5;
/// Peak horizontal displacement of the back layer, in pixels (= cells)
/// either side. Each layer forward swings DEPTH wider than the one behind.
const SWING: f64 = 10.0;
const DEPTH: f64 = 0.18;
/// Phase lag between neighbouring layers, as a fraction of PERIOD.
const LAG: f64 = 0.11;

/// A hump-ridged layer: everything below the ridge line is filled with
/// a dither of `fill.0`/`fill.1` at level `fill.2`.
/// `base` = valley floor (pixel row), `amp` = hump height, `wl` = two
/// hump widths mixed for irregular ridges.
fn ridge(base: f64, amp: f64, wl: (f64, f64), seed: f64, fill: (Color, Color, u8)) -> Layer {
    ridge_lifted(base, amp, wl, seed, fill, |_| 0.0)
}

/// `ridge`, with the ridge line raised by `lift(x)` pixels.
fn ridge_lifted(
    base: f64,
    amp: f64,
    wl: (f64, f64),
    seed: f64,
    fill: (Color, Color, u8),
    lift: impl Fn(f64) -> f64 + Send + 'static,
) -> Layer {
    // The ridge line depends only on x: compute it once per column, then
    // fill straight down.
    Layer::paint(move |c| {
        for sx in 0..c.width {
            let x = sx as f64 - c.offset;
            let a = (PI * x / wl.0 + seed).sin().abs();
            let b = (PI * x / wl.1 + seed * 1.7).sin().abs();
            let top = base - amp * (0.65 * a + 0.35 * b) - lift(x);
            for y in (top.ceil().max(0.0) as usize)..c.height {
                c.set(sx as i64, y as i64, dither(fill.0, fill.1, fill.2, sx, y));
            }
        }
    })
}

/// Chunky Mario-title-style capitals, drawn for this demo: 7×8, three-
/// pixel stems, `#` = lit.
const MARIO_GLYPHS: [(char, [&str; 8]); 10] = [
    ('A', [".#####.", "#######", "###.###", "###.###", "#######", "#######", "###.###", "###.###"]),
    ('C', [".######", "#######", "###....", "###....", "###....", "###....", "#######", ".######"]),
    ('H', ["###.###", "###.###", "###.###", "#######", "#######", "###.###", "###.###", "###.###"]),
    ('I', ["#######", "#######", "..###..", "..###..", "..###..", "..###..", "#######", "#######"]),
    ('K', ["###.###", "###.##.", "#####..", "####...", "#####..", "###.##.", "###.###", "###.###"]),
    ('L', ["###....", "###....", "###....", "###....", "###....", "###....", "#######", "#######"]),
    ('M', ["##...##", "###.###", "#######", "#######", "##.#.##", "##...##", "##...##", "##...##"]),
    ('O', [".#####.", "#######", "###.###", "###.###", "###.###", "###.###", "#######", ".#####."]),
    ('P', ["######.", "#######", "###.###", "###.###", "#######", "######.", "###....", "###...."]),
    ('R', ["######.", "#######", "###.###", "###.###", "######.", "#####..", "###.###", "###.###"]),
];

const WORD: &str = "CHROMAKOPIA";

/// How far above centre the wordmark sits, in pixels — unless its DYCP
/// swing needs more headroom.
const LOGO_LIFT: i64 = 15;
/// DYCP swing grows with letter scale, but no more than this many times.
const DYCP_MAX_GROW: f64 = 2.0;

/// Glyph advance: 7 wide + 1, so neighbouring faces sit snug against
/// each other with exactly one shared outline pixel between them.
const ADVANCE: usize = 8;

// DYCP: two sines running through the word in opposite directions —
// the main wave (DYCP_PERIOD, DYCP_SPREAD radians per letter) and a
// slower counter-wave — so the shape keeps changing, with the whole
// swing breathing bigger and smaller every DYCP_BREATHE seconds.
// Peaks reach about ±1.3 × DYCP_AMP glyph pixels.
const DYCP_AMP: f64 = 5.0;
const DYCP_PERIOD: f64 = 1.2;
const DYCP_SPREAD: f64 = 0.7;
const DYCP_BREATHE: f64 = 4.0;

// Entrance: letter i starts falling at DROP_START + i × DROP_STAGGER
// seconds, from FALL pixels up, landing with a bounce DROP_SECS later;
// its DYCP swell then ramps in over DYCP_RAMP seconds.
const DROP_START: f64 = 0.6;
const DROP_STAGGER: f64 = 0.12;
const DROP_SECS: f64 = 0.9;
const FALL: f64 = (ROWS * 2) as f64;
const DYCP_RAMP: f64 = 1.0;

/// Stamp one 8×8 glyph (MSB = leftmost pixel) at `scale`× with its top-
/// left at (ox, oy): `extrude` steps of extrusion down-right, a one-pixel
/// outline, then the face — cream along each top edge, pink upper half,
/// lavender lower. `face` is scratch space, reused between calls.
fn stamp(
    c: &mut Canvas,
    glyph: &[u8; 8],
    (ox, oy): (i64, i64),
    scale: i64,
    extrude: i64,
    face: &mut Vec<(i64, i64, Color)>,
) {
    let lit = |gx: i64, gy: i64| (0..8).contains(&gy) && (0..8).contains(&gx) && glyph[gy as usize] & (0x80 >> gx) != 0;
    face.clear();
    for gy in 0..8 {
        for gx in 0..8 {
            if !lit(gx, gy) {
                continue;
            }
            for dy in 0..scale {
                let color = if dy == 0 && !lit(gx, gy - 1) {
                    LETTER_CREAM
                } else if gy < 4 {
                    LETTER_PINK
                } else {
                    LETTER_LAVENDER
                };
                for dx in 0..scale {
                    face.push((ox + gx * scale + dx, oy + gy * scale + dy, color));
                }
            }
        }
    }
    let shells = (1..=extrude).rev().map(|k| (k, LETTER_EXTRUDE)).chain([(0, LETTER_OUTLINE)]);
    for (shift, color) in shells {
        for &(x, y, _) in face.iter() {
            for ny in -1..=1 {
                for nx in -1..=1 {
                    c.set(x + nx + shift, y + ny + shift, color);
                }
            }
        }
    }
    for &(x, y, color) in face.iter() {
        c.set(x, y, color);
    }
}

/// The wordmark: glyphs at `scale`× size, each letter at its own height.
struct Bitmap {
    glyphs: Vec<[u8; 8]>,
    scale: usize,
}

impl Bitmap {
    fn new(text: &str, scale: usize) -> Self {
        let glyphs = text
            .chars()
            .map(|c| {
                let rows = MARIO_GLYPHS.iter().find(|(g, _)| *g == c).expect("glyph in MARIO_GLYPHS").1;
                rows.map(|row| row.bytes().enumerate().fold(0, |m, (i, b)| if b == b'#' { m | 0x80 >> i } else { m }))
            })
            .collect();
        Self { glyphs, scale }
    }

    fn width(&self) -> usize {
        ((self.glyphs.len() - 1) * ADVANCE + 7) * self.scale
    }

    fn height(&self) -> usize {
        8 * self.scale
    }

    /// Largest upward DYCP excursion, in screen pixels.
    fn max_swing(&self) -> i64 {
        (1.32 * DYCP_AMP * self.swing_scale()).ceil() as i64
    }

    fn swing_scale(&self) -> f64 {
        (self.scale as f64).min(DYCP_MAX_GROW)
    }

    /// Letter `i`'s vertical offset in screen pixels: bounce-in drop,
    /// then its DYCP sine.
    fn bob(&self, i: usize, frame: usize) -> i64 {
        let t = frame as f64 / FPS;
        let falling = t - (DROP_START + i as f64 * DROP_STAGGER);
        let drop = -FALL * (1.0 - Easing::BounceOut.apply(falling / DROP_SECS));
        let swell = Easing::EaseInOut.apply((falling - DROP_SECS) / DYCP_RAMP);
        let i = i as f64;
        let main = (TAU * t / DYCP_PERIOD - i * DYCP_SPREAD).sin();
        let counter = (TAU * t / (DYCP_PERIOD * 1.9) + i * DYCP_SPREAD * 0.6).sin();
        let breathe = 0.8 + 0.35 * (TAU * t / DYCP_BREATHE).sin();
        let s = (0.7 * main + 0.45 * counter) * breathe;
        (drop + swell * DYCP_AMP * self.swing_scale() * s).round() as i64
    }

    /// Paint the word with its top-left at (x0, y0). Letters go left to
    /// right, each fully (extrusion, outline, face) before the next, so
    /// every letter covers its left neighbour's extrusion.
    fn paint(&self, c: &mut Canvas, x0: i64, y0: i64) {
        let s = self.scale as i64;
        let mut face = Vec::new();
        for (i, glyph) in self.glyphs.iter().enumerate() {
            let at = (x0 + (i * ADVANCE) as i64 * s, y0 + self.bob(i, c.frame));
            stamp(c, glyph, at, s, 2, &mut face);
        }
    }
}

// Scroller: one pixel per frame (smooth at any FPS-locked rate), riding a
// gentle sine through screen x — classic cracktro bottom scroller.
const SCROLL_SPEED: f64 = FPS;
const SCROLL_WAVE: f64 = 4.0;
const SCROLL_WAVE_PERIOD: f64 = 0.8;

/// The licence, in C64 glyphs, scrolling right to left along the bottom.
struct Scroller {
    glyphs: Vec<[u8; 8]>,
    y0: i64,
}

impl Scroller {
    fn new(text: &str, y0: i64) -> Self {
        let flat = text.replace("\n\n", "   ").replace('\n', " ");
        let glyphs = flat
            .chars()
            .map(|c| C64_GLYPHS.iter().find(|(g, _)| *g == c).expect("glyph in C64_GLYPHS").1)
            .collect();
        Self { glyphs, y0 }
    }

    /// Only the glyphs on screen are stamped. The text enters from the
    /// right edge, runs out, and starts again.
    fn paint(&self, c: &mut Canvas) {
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

/// C64 character ROM, mixed-case set (second 2 KB bank: a–z = screen
/// codes 1–26, A–Z = 65–90), 8×8, MSB = leftmost pixel — just the
/// characters the scroller needs. Bytes from `cbmeeks/VgaPico`
/// `fonts/petscii/chargen`.
const C64_GLYPHS: [(char, [u8; 8]); 58] = [
    (' ', [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('"', [0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('(', [0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00]),
    (')', [0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00]),
    (',', [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00]),
    ('/', [0x00, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x00]),
    ('0', [0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00]),
    ('2', [0x3C, 0x66, 0x06, 0x0C, 0x30, 0x60, 0x7E, 0x00]),
    ('6', [0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00]),
    (':', [0x00, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x00]),
    ('A', [0x18, 0x3C, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]),
    ('B', [0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00]),
    ('C', [0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00]),
    ('D', [0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00]),
    ('E', [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x7E, 0x00]),
    ('F', [0x7E, 0x60, 0x60, 0x78, 0x60, 0x60, 0x60, 0x00]),
    ('G', [0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3C, 0x00]),
    ('H', [0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]),
    ('I', [0x3C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00]),
    ('J', [0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00]),
    ('K', [0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00]),
    ('L', [0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00]),
    ('M', [0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00]),
    ('N', [0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00]),
    ('O', [0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00]),
    ('P', [0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00]),
    ('R', [0x7C, 0x66, 0x66, 0x7C, 0x78, 0x6C, 0x66, 0x00]),
    ('S', [0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00]),
    ('T', [0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00]),
    ('U', [0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00]),
    ('V', [0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00]),
    ('W', [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00]),
    ('X', [0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00]),
    ('Y', [0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00]),
    ('a', [0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00]),
    ('b', [0x00, 0x60, 0x60, 0x7C, 0x66, 0x66, 0x7C, 0x00]),
    ('c', [0x00, 0x00, 0x3C, 0x60, 0x60, 0x60, 0x3C, 0x00]),
    ('d', [0x00, 0x06, 0x06, 0x3E, 0x66, 0x66, 0x3E, 0x00]),
    ('e', [0x00, 0x00, 0x3C, 0x66, 0x7E, 0x60, 0x3C, 0x00]),
    ('f', [0x00, 0x0E, 0x18, 0x3E, 0x18, 0x18, 0x18, 0x00]),
    ('g', [0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x7C]),
    ('h', [0x00, 0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x00]),
    ('i', [0x00, 0x18, 0x00, 0x38, 0x18, 0x18, 0x3C, 0x00]),
    ('j', [0x00, 0x06, 0x00, 0x06, 0x06, 0x06, 0x06, 0x3C]),
    ('k', [0x00, 0x60, 0x60, 0x6C, 0x78, 0x6C, 0x66, 0x00]),
    ('l', [0x00, 0x38, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00]),
    ('m', [0x00, 0x00, 0x66, 0x7F, 0x7F, 0x6B, 0x63, 0x00]),
    ('n', [0x00, 0x00, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00]),
    ('o', [0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00]),
    ('p', [0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60]),
    ('r', [0x00, 0x00, 0x7C, 0x66, 0x60, 0x60, 0x60, 0x00]),
    ('s', [0x00, 0x00, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x00]),
    ('t', [0x00, 0x18, 0x7E, 0x18, 0x18, 0x18, 0x0E, 0x00]),
    ('u', [0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3E, 0x00]),
    ('v', [0x00, 0x00, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00]),
    ('w', [0x00, 0x00, 0x63, 0x6B, 0x7F, 0x3E, 0x36, 0x00]),
    ('y', [0x00, 0x00, 0x66, 0x66, 0x66, 0x3E, 0x0C, 0x78]),
];

const LICENSE_TEXT: &str = r#"MIT License

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

/// A towering cumulus every 150 pixels: a dome 15 pixels tall and ~36
/// wide with a scalloped top, lifting the front cloud layer up past the
/// back one into the open sky.
fn cumulus(x: f64) -> f64 {
    let dx = (x.rem_euclid(150.0) - 60.0) / 18.0;
    let dome = (1.0 - dx * dx).max(0.0).sqrt();
    15.0 * dome * (0.8 + 0.2 * (PI * x / 6.0).sin().abs())
}

#[tokio::main]
async fn main() {
    // Landscape fills everything below the TOP_PAD sky rows.
    let top = (TOP_PAD * 2) as f64;
    let h = ((ROWS - TOP_PAD) * 2) as f64;
    // Back to front: clouds, greens, then ground.
    let ridges = [
        ridge(top + h * 0.28, 8.0, (23.0, 41.0), 0.3, (CLOUD_LILAC, CLOUD_PINK, 2)),
        ridge_lifted(top + h * 0.38, 8.0, (19.0, 37.0), 1.9, (CLOUD_BLUSH, CLOUD_BLUSH, 0), cumulus),
        ridge(top + h * 0.49, 8.0, (21.0, 33.0), 4.1, (MINT_MIST, MINT_PALE, 2)),
        ridge(top + h * 0.59, 7.0, (25.0, 43.0), 2.6, (MINT, PISTACHIO, 1)),
        ridge(top + h * 0.69, 7.0, (18.0, 31.0), 5.2, (SAGE, MINT, 1)),
        ridge(top + h * 0.79, 6.0, (22.0, 39.0), 0.9, (SAGE_DEEP, SAGE, 2)),
        ridge(top + h * 0.91, 6.0, (20.0, 35.0), 3.3, (CORAL, PEACH, 1)),
        ridge(top + h * 1.03, 5.0, (24.0, 45.0), 6.0, (ROSE, CORAL, 2)),
    ];

    // Sky: pastel blue, dithered toward periwinkle in the top rows.
    let sky = Layer::new(|p| {
        let level = match p.y {
            0..=1 => 2,
            2..=3 => 1,
            _ => 0,
        };
        Some(p.dither(SKY, SKY_HAZE, level))
    });

    // Cloud shadows: ovals over the land, one every 64 pixels, each a
    // bit different in size and height — multiplied with NES blue so they
    // cool whatever they pass over. Slower and wider than the ridges, on
    // their own swing.
    let shadows = Layer::new(move |p| {
        let slot = (p.x / 64.0).floor();
        let vary = (slot * 2.3).sin(); // -1..1, fixed per oval
        let dx = (p.x - slot * 64.0 - 32.0) / (16.0 + 5.0 * vary);
        let dy = (p.y as f64 - top - h * (0.72 + 0.06 * vary)) / 6.0;
        (dx * dx + dy * dy < 1.0).then_some(SHADOW_BLUE)
    })
    .swing(30.0, 9.0, Easing::EaseInOut)
    .blend(BlendMode::Multiply)
    .opacity(0.3);

    let mut layers = Layers::new(ROWS).layer(sky);
    for (i, r) in ridges.into_iter().enumerate() {
        let r = r
            .swing(SWING * (1.0 + DEPTH * i as f64), PERIOD, Easing::EaseInOut)
            .phase(-(i as f64) * LAG);
        // The first two ridges are the clouds — slightly see-through.
        let r = if i < 2 { r.opacity(0.85) } else { r };
        layers = layers.layer(r);
    }
    let layers = layers.layer(shadows);

    // The wordmark, centred. Scale up to whatever fits the terminal.
    let screen_w = terminal_width();
    let word = Bitmap::new(WORD, (screen_w / (WORD.len() * ADVANCE + 8)).clamp(1, 3));
    let x0 = (screen_w as i64 - word.width() as i64) / 2;
    // Centred, then lifted LOGO_LIFT pixels — but never so high that the
    // tallest DYCP swing (plus outline) leaves the top of the screen.
    let y0 = ((ROWS * 2) as i64 / 2 - word.height() as i64 / 2 - LOGO_LIFT).max(word.max_swing() + 3);
    // Plum outline, violet extrusion, cream/pink/lavender face — all
    // painted by the one wordmark layer.
    let layers = layers.layer(Layer::paint(move |c| word.paint(c, x0, y0)));

    // The licence scrolls along the bottom, over the ground.
    // Baseline high enough that the ±SCROLL_WAVE swing plus extrusion
    // stays on screen.
    let scroller = Scroller::new(LICENSE_TEXT, (ROWS * 2) as i64 - 15);
    let layers = layers.layer(Layer::paint(move |c| scroller.paint(c)));

    Scene::new()
        .add(layers)
        .fullscreen()
        .quit_key('q')
        .run(3600.0)
        .await;
}
