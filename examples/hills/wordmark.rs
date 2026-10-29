//! The CHROMAKOPIA wordmark: Mario-style letters that drop in and ride a DYCP wave.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{mario, stamp};
use super::ROWS;

pub const WORD: &str = "CHROMAKOPIA";

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

// Exit: letter i rises off the top starting at i × EXIT_STAGGER seconds,
// taking EXIT_SECS (ease-in) — 11 letters finish within 1.5 s.
pub const EXIT_STAGGER: f64 = 0.08;
pub const EXIT_SECS: f64 = 0.6;

/// What the wordmark is doing, with local seconds into that phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogoPhase {
    Hidden,
    Enter(f64),
    Exit(f64),
}

/// Letter `i`'s drop-in offset `t` seconds into the entrance (−FALL → 0).
fn drop_offset(i: usize, t: f64) -> f64 {
    let falling = t - (DROP_START + i as f64 * DROP_STAGGER);
    -FALL * (1.0 - Easing::BounceOut.apply(falling / DROP_SECS))
}

/// Letter `i`'s lift `t` seconds into the exit (0 → −FALL).
fn exit_offset(i: usize, t: f64) -> f64 {
    -FALL * Easing::EaseIn.apply((t - i as f64 * EXIT_STAGGER) / EXIT_SECS)
}

/// The wordmark: glyphs at `scale`× size, each letter at its own height.
pub struct Wordmark {
    glyphs: Vec<[u8; 8]>,
    scale: usize,
}

impl Wordmark {
    /// The wordmark sized and placed for a screen `screen_w` columns wide:
    /// (wordmark, x0, y0).
    pub fn placed(screen_w: usize) -> (Wordmark, i64, i64) {
        let word = Wordmark::new(WORD, (screen_w / (WORD.len() * ADVANCE + 8)).clamp(1, 3));
        let x0 = (screen_w as i64 - word.width() as i64) / 2;
        // Centred, then lifted LOGO_LIFT pixels — but never so high that the
        // tallest DYCP swing (plus outline) leaves the top of the screen.
        let y0 = ((ROWS * 2) as i64 / 2 - word.height() as i64 / 2 - LOGO_LIFT).max(word.max_swing() + 3);
        (word, x0, y0)
    }

    fn new(text: &str, scale: usize) -> Self {
        let glyphs = text
            .chars()
            .map(mario)
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

    /// Letter `i`'s vertical offset: phase movement plus the DYCP wave,
    /// which runs on wall time so it never jumps between phases.
    fn bob(&self, i: usize, wall: f64, phase: LogoPhase) -> i64 {
        let (shift, swell) = match phase {
            LogoPhase::Hidden => return 0,
            LogoPhase::Enter(t) => {
                let landed = t - (DROP_START + i as f64 * DROP_STAGGER) - DROP_SECS;
                (drop_offset(i, t), Easing::EaseInOut.apply(landed / DYCP_RAMP))
            }
            LogoPhase::Exit(t) => (exit_offset(i, t), 1.0),
        };
        let i = i as f64;
        let main = (TAU * wall / DYCP_PERIOD - i * DYCP_SPREAD).sin();
        let counter = (TAU * wall / (DYCP_PERIOD * 1.9) + i * DYCP_SPREAD * 0.6).sin();
        let breathe = 0.8 + 0.35 * (TAU * wall / DYCP_BREATHE).sin();
        let s = (0.7 * main + 0.45 * counter) * breathe;
        (shift + swell * DYCP_AMP * self.swing_scale() * s).round() as i64
    }

    /// Paint the word with its top-left at (x0, y0). Letters go left to
    /// right, each fully (extrusion, outline, face) before the next, so
    /// every letter covers its left neighbour's extrusion.
    pub fn paint(&self, c: &mut Canvas, x0: i64, y0: i64, phase: LogoPhase) {
        if phase == LogoPhase::Hidden {
            return;
        }
        let wall = c.frame as f64 / FPS;
        let s = self.scale as i64;
        let mut face = Vec::new();
        for (i, glyph) in self.glyphs.iter().enumerate() {
            let at = (x0 + (i * ADVANCE) as i64 * s, y0 + self.bob(i, wall, phase));
            stamp(c, glyph, at, s, 2, &mut face);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_starts_off_screen_and_lands() {
        assert_eq!(drop_offset(0, 0.0), -FALL);
        assert!(drop_offset(10, DROP_START + 10.0 * DROP_STAGGER + DROP_SECS).abs() < 1e-6);
    }

    #[test]
    fn exit_starts_in_place_and_ends_off_screen() {
        assert_eq!(exit_offset(0, 0.0), 0.0);
        for i in 0..WORD.len() {
            assert_eq!(exit_offset(i, super::super::stage::LOGO_EXIT_SECS), -FALL);
        }
    }

    #[test]
    fn exit_is_staggered() {
        let t = 0.3;
        assert!(exit_offset(0, t) < exit_offset(5, t));
    }
}
