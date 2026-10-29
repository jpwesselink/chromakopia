//! The CHROMAKOPIA wordmark: Mario-style letters that drop in and ride a
//! DYCP wave, then rise to the top of the screen and stay there, calmer,
//! above the effect panels.

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

/// Seconds to glide between the centre and the top.
pub const RISE_SECS: f64 = super::stage::LOGO_RISE_SECS;
/// DYCP swing at the top, as a fraction of the full swing — calm enough
/// to leave room for the panels below.
const CALM: f64 = 0.22;
/// Topmost pixel row the wordmark may reach (row 0 is the border).
const TOP_MARGIN: i64 = 2;

/// What the wordmark is doing, with local seconds into that phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogoPhase {
    /// Dropping in at the centre, full swing.
    Enter(f64),
    /// Gliding from the centre up to the top.
    Rise(f64),
    /// At the top, calm swing.
    Top,
    /// Gliding from the top back down to the centre.
    Fall(f64),
}

impl LogoPhase {
    /// 0 = centre with full swing, 1 = top with calm swing.
    fn lift(self) -> f64 {
        match self {
            LogoPhase::Enter(_) => 0.0,
            LogoPhase::Rise(t) => Easing::EaseInOut.apply(t / RISE_SECS),
            LogoPhase::Top => 1.0,
            LogoPhase::Fall(t) => 1.0 - Easing::EaseInOut.apply(t / RISE_SECS),
        }
    }
}

/// Letter `i`'s drop-in offset `t` seconds into the entrance (−FALL → 0).
fn drop_offset(i: usize, t: f64) -> f64 {
    let falling = t - (DROP_START + i as f64 * DROP_STAGGER);
    -FALL * (1.0 - Easing::BounceOut.apply(falling / DROP_SECS))
}

/// The wordmark: glyphs at `scale`× size, each letter at its own height,
/// placed for one screen width.
pub struct Wordmark {
    glyphs: Vec<[u8; 8]>,
    scale: usize,
    x0: i64,
    /// Top pixel row at the centre (intro, finale) and at the top.
    y_mid: i64,
    y_top: i64,
}

impl Wordmark {
    /// The wordmark sized and placed for a screen `screen_w` columns wide.
    /// At most 2× so it fits above the panels.
    pub fn placed(screen_w: usize) -> Wordmark {
        let scale = (screen_w / (WORD.len() * ADVANCE + 8)).clamp(1, 2);
        let glyphs = WORD.chars().map(mario).collect();
        let mut word = Self { glyphs, scale, x0: 0, y_mid: 0, y_top: 0 };
        word.x0 = (screen_w as i64 - word.width() as i64) / 2;
        // Centred, then lifted LOGO_LIFT pixels — but never so high that the
        // tallest DYCP swing (plus outline) leaves the top of the screen.
        word.y_mid = ((ROWS * 2) as i64 / 2 - word.height() as i64 / 2 - LOGO_LIFT).max(word.max_swing() + 3);
        word.y_top = TOP_MARGIN + word.calm_swing() + 1;
        word
    }

    /// Lowest pixel row the wordmark reaches at the top: glyph, two-step
    /// extrusion, calm swing down.
    #[cfg(test)]
    pub fn bottom_at_top(&self) -> i64 {
        self.y_top + self.height() as i64 + 2 + self.calm_swing()
    }

    fn calm_swing(&self) -> i64 {
        (1.32 * DYCP_AMP * self.swing_scale() * CALM).ceil() as i64
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
            LogoPhase::Enter(t) => {
                let landed = t - (DROP_START + i as f64 * DROP_STAGGER) - DROP_SECS;
                (drop_offset(i, t), Easing::EaseInOut.apply(landed / DYCP_RAMP))
            }
            _ => (0.0, 1.0 + (CALM - 1.0) * phase.lift()),
        };
        let i = i as f64;
        let main = (TAU * wall / DYCP_PERIOD - i * DYCP_SPREAD).sin();
        let counter = (TAU * wall / (DYCP_PERIOD * 1.9) + i * DYCP_SPREAD * 0.6).sin();
        let breathe = 0.8 + 0.35 * (TAU * wall / DYCP_BREATHE).sin();
        let s = (0.7 * main + 0.45 * counter) * breathe;
        (shift + swell * DYCP_AMP * self.swing_scale() * s).round() as i64
    }

    /// Paint the word for `phase`. Letters go left to right, each fully
    /// (extrusion, outline, face) before the next, so every letter covers
    /// its left neighbour's extrusion.
    pub fn paint(&self, c: &mut Canvas, phase: LogoPhase) {
        let (x0, lift) = (self.x0, phase.lift());
        let y0 = (self.y_mid as f64 + (self.y_top - self.y_mid) as f64 * lift).round() as i64;
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
    fn rises_to_the_top_and_stays_on_screen_there() {
        for screen_w in [80, 120, 200, 300] {
            let word = Wordmark::placed(screen_w);
            let bottom = word.bottom_at_top();
            let layers = Layers::new(ROWS).layer(Layer::paint(move |c| word.paint(c, LogoPhase::Top)));
            let (mut lowest, mut highest) = (0, i64::MAX);
            for frame in (0..600).step_by(3) {
                let mut buf = FrameBuffer::new(screen_w, ROWS);
                layers.render(&mut buf, frame);
                for y in 0..ROWS {
                    for x in 0..screen_w {
                        let cell = buf.get(x, y);
                        let (top, low) = match (cell.ch, cell.bg.is_some()) {
                            ('▀', true) => (true, true),
                            ('▀', false) => (true, false),
                            ('▄', _) => (false, true),
                            _ => (false, false),
                        };
                        let y = y as i64 * 2;
                        if top { highest = highest.min(y); lowest = lowest.max(y); }
                        if low { highest = highest.min(y + 1); lowest = lowest.max(y + 1); }
                    }
                }
            }
            assert!(highest >= TOP_MARGIN, "{screen_w}: reaches row {highest}");
            assert!(lowest <= bottom, "{screen_w}: reaches {lowest} > {bottom}");
        }
    }

    #[test]
    fn rise_and_fall_glide_between_centre_and_top() {
        assert_eq!(LogoPhase::Rise(0.0).lift(), 0.0);
        assert_eq!(LogoPhase::Rise(RISE_SECS).lift(), 1.0);
        assert_eq!(LogoPhase::Fall(RISE_SECS).lift(), 0.0);
        assert_eq!(LogoPhase::Top.lift(), 1.0);
    }
}
