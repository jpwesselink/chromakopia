//! The CHROMAKOPIA wordmark: Mario-style letters that drop in and ride a
//! DYCP wave, then rise to the top of the screen and stay there, calmer,
//! above the effect panels.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{mario, stamp, stamp_tinted, LETTER_LOWER, LETTER_UPPER};
use super::layout::Layout;
use super::dive;
use super::looping;
use super::tiny;
use super::ROWS;

pub const WORD: &str = "CHROMAKOPIA";
/// The line under the wordmark, typed out once the letters have landed.
pub const TAGLINE: &str = "terminal eye candy";
/// Characters per second the tagline types at.
const TYPE_RATE: f64 = 24.0;
/// When the tagline starts typing, seconds into the entrance.
const TYPE_START: f64 = 2.8;

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

// Exit: letter i jumps off the top starting at i × EXIT_STAGGER seconds,
// taking EXIT_SECS (ease-in).
const EXIT_STAGGER: f64 = 0.08;
const EXIT_SECS: f64 = 0.6;
/// Seconds until the last letter is gone.
pub const EXIT_DONE: f64 = (WORD.len() - 1) as f64 * EXIT_STAGGER + EXIT_SECS;

/// The C left behind by `{logo away}`: how many times its size it grows
/// to in the middle of the loop, how long that takes, and how much of the
/// full DYCP bob it keeps there.
pub const BIG_C: i64 = 3;
pub const MORPH_SECS: f64 = 1.0;
const BIG_C_BOB: f64 = 0.3;
/// The liquid in the big C: how high its waves slosh, in pixels.
pub const SLOSH: f64 = 1.5;
/// When the C leaves for the loop: as the last of the others (H) jumps.
const C_LEAVES: f64 = (WORD.len() - 2) as f64 * EXIT_STAGGER;
/// Seconds until the others are gone and the C is big in the loop.
#[cfg(test)]
pub const AWAY_DONE: f64 = C_LEAVES + MORPH_SECS;

/// Seconds to glide between the centre and the top.
pub const RISE_SECS: f64 = super::stage::LOGO_RISE_SECS;
/// Peak of the DYCP sine mix, as a multiple of DYCP_AMP.
const SWING_PEAK: f64 = 1.3225;

/// Letter scale for a screen `screen_w` columns wide: at most 2× so it
/// fits above the panels.
pub fn scale_for(screen_w: usize) -> usize {
    (screen_w / (WORD.len() * ADVANCE + 8)).clamp(1, 2)
}

/// Glyph height in pixels at `scale`.
pub fn height(scale: usize) -> i64 {
    8 * scale as i64
}

/// Largest DYCP excursion at full swing, in pixels, at `scale`.
pub fn full_swing(scale: usize) -> i64 {
    (SWING_PEAK * DYCP_AMP * (scale as f64).min(DYCP_MAX_GROW)).ceil() as i64
}

/// What the wordmark is doing, with local seconds into that phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogoPhase {
    /// Not on screen.
    Hidden,
    /// Dropping in at the centre, full swing.
    Enter(f64),
    /// Gliding from the centre up to the top.
    Rise(f64),
    /// At the top, calm swing.
    Top,
    /// Gliding from the top back down to the centre.
    Fall(f64),
    /// Jumping off the top, letter by letter, from the centre.
    Exit(f64),
    /// Stepping aside for the loop: every letter but the C jumps off, from
    /// the end of the word back; then the C glides into the middle of the
    /// loop and grows big there.
    Away(f64),
    /// Back after the loop: the C shrinks home while the rest drop in.
    Return(f64),
}

impl LogoPhase {
    /// 0 = centre with full swing, 1 = top with calm swing.
    fn lift(self) -> f64 {
        match self {
            LogoPhase::Hidden | LogoPhase::Enter(_) | LogoPhase::Exit(_) | LogoPhase::Away(_) | LogoPhase::Return(_) => 0.0,
            LogoPhase::Rise(t) => Easing::EaseInOut.apply(t / RISE_SECS),
            LogoPhase::Top => 1.0,
            LogoPhase::Fall(t) => 1.0 - Easing::EaseInOut.apply(t / RISE_SECS),
        }
    }
}

/// Letter `i`'s lift `t` seconds into the exit (0 → −FALL).
fn exit_offset(i: usize, t: f64) -> f64 {
    -FALL * Easing::EaseIn.apply(((t - i as f64 * EXIT_STAGGER) / EXIT_SECS).clamp(0.0, 1.0))
}

/// Letter `i`'s lift `t` seconds into stepping aside: the exit, but from
/// the end of the word back, so the A goes first and the H last.
fn away_offset(i: usize, t: f64) -> f64 {
    exit_offset(WORD.len() - 1 - i, t)
}

/// How far the C has grown into the loop's middle in `phase` (0 = home).
/// How far the big C bobs up and down in the loop, `wall` seconds in.
pub fn big_c_bob(wall: f64) -> f64 {
    BIG_C_BOB * DYCP_AMP * (TAU * wall / DYCP_PERIOD).sin()
}

fn c_grown(phase: LogoPhase) -> f64 {
    match phase {
        LogoPhase::Away(t) => Easing::EaseInOut.apply(((t - C_LEAVES) / MORPH_SECS).clamp(0.0, 1.0)),
        LogoPhase::Return(t) => 1.0 - Easing::EaseInOut.apply((t / MORPH_SECS).clamp(0.0, 1.0)),
        _ => 0.0,
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
    /// Swing at the top as a fraction of the full swing.
    calm: f64,
    /// Tagline glyph top at the centre and at the top.
    tag_mid: i64,
    tag_top: i64,
}

impl Wordmark {
    /// The wordmark sized and placed for a screen `screen_w` columns wide.
    pub fn placed(screen_w: usize) -> Wordmark {
        let layout = Layout::for_width(screen_w);
        let scale = layout.scale;
        let glyphs = WORD.chars().map(mario).collect();
        let full = full_swing(scale);
        let mut word = Self { glyphs, scale, x0: 0, y_mid: 0, y_top: layout.logo_y, calm: 0.0, tag_mid: 0, tag_top: layout.tagline_y };
        word.x0 = (screen_w as i64 - word.width() as i64) / 2;
        // Centred, then lifted LOGO_LIFT pixels — but never so high that the
        // tallest DYCP swing (plus outline) leaves the top of the screen.
        word.y_mid = ((ROWS * 2) as i64 / 2 - height(scale) / 2 - LOGO_LIFT).max(full + 3);
        word.tag_mid = word.y_mid + height(scale) + 2 + full + 2;
        word.calm = layout.top_swing as f64 / (SWING_PEAK * DYCP_AMP * word.swing_scale());
        word
    }

    /// Paint the tagline under the wordmark for `phase`: typed out during
    /// the entrance, then riding along with the wordmark.
    pub fn paint_tagline(&self, c: &mut Canvas, phase: LogoPhase) {
        if phase == LogoPhase::Hidden {
            return;
        }
        let shown = match phase {
            LogoPhase::Enter(t) | LogoPhase::Return(t) => ((t - TYPE_START).max(0.0) * TYPE_RATE) as usize,
            // Backspaced away, three times as fast as it was typed.
            LogoPhase::Exit(t) | LogoPhase::Away(t) => TAGLINE.len().saturating_sub((t * TYPE_RATE * 3.0) as usize),
            _ => TAGLINE.len(),
        };
        let lift = phase.lift();
        let y = (self.tag_mid as f64 + (self.tag_top - self.tag_mid) as f64 * lift).round() as i64;
        let x = (c.width as i64 - tiny::width(TAGLINE)) / 2;
        tiny::stamp(c, TAGLINE, (x, y), shown);
    }

    fn width(&self) -> usize {
        ((self.glyphs.len() - 1) * ADVANCE + 7) * self.scale
    }


    fn swing_scale(&self) -> f64 {
        (self.scale as f64).min(DYCP_MAX_GROW)
    }

    /// Letter `i`'s vertical offset: phase movement plus the DYCP wave,
    /// which runs on wall time so it never jumps between phases.
    fn bob(&self, i: usize, wall: f64, phase: LogoPhase) -> i64 {
        let (shift, swell) = match phase {
            // The C comes home from the loop rather than dropping in.
            LogoPhase::Return(t) if i == 0 => (0.0, Easing::EaseInOut.apply((t - MORPH_SECS) / DYCP_RAMP)),
            LogoPhase::Enter(t) | LogoPhase::Return(t) => {
                let landed = t - (DROP_START + i as f64 * DROP_STAGGER) - DROP_SECS;
                (drop_offset(i, t), Easing::EaseInOut.apply(landed / DYCP_RAMP))
            }
            LogoPhase::Exit(t) => (exit_offset(i, t), 1.0),
            // The C stays put: it leaves by gliding into the loop instead.
            LogoPhase::Away(_) if i == 0 => (0.0, 1.0),
            LogoPhase::Away(t) => (away_offset(i, t), 1.0),
            _ => (0.0, 1.0 + (self.calm - 1.0) * phase.lift()),
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
    /// its left neighbour's extrusion. `tilt` is which way is down in the
    /// big C, if it's out (see `looping::tilt`); `dive` how far the camera
    /// has gone into it (see `dive`).
    pub fn paint(&self, c: &mut Canvas, phase: LogoPhase, tilt: f64, dive: dive::Dive) {
        if phase == LogoPhase::Hidden {
            return;
        }
        let (x0, lift) = (self.x0, phase.lift());
        let y0 = (self.y_mid as f64 + (self.y_top - self.y_mid) as f64 * lift).round() as i64;
        let wall = c.frame as f64 / FPS;
        let s = self.scale as i64;
        let mut face = Vec::new();
        for (i, glyph) in self.glyphs.iter().enumerate() {
            let at = (x0 + (i * ADVANCE) as i64 * s, y0 + self.bob(i, wall, phase));
            let grown = c_grown(phase);
            if i == 0 && grown >= 1.0 && dive.zoom > 0.0 {
                dive::Camera::new(c.width, c.height, dive.zoom, wall).paint(c, glyph, wall, dive.waves, tilt);
            } else if i == 0 && grown > 0.0 {
                self.paint_big_c(c, glyph, at, grown, wall, tilt);
            } else {
                stamp(c, glyph, at, s, 2, &mut face);
            }
        }
    }

    /// The C `grown` (0..=1) of the way from its place in the word, top-left
    /// at `home`, to BIG_C times its size in the middle of the loop —
    /// growing a pixel at a time as it glides. It's half full: lavender
    /// liquid under pink air, sloshing, its surface level across `tilt` —
    /// down turning with the letters riding the loop.
    fn paint_big_c(&self, c: &mut Canvas, glyph: &[u8; 8], home: (i64, i64), grown: f64, wall: f64, tilt: f64) {
        let (small, big) = (8.0 * self.scale as f64, 8.0 * BIG_C as f64);
        let (cx, cy) = looping::loop_centre(c.width);
        let bob = big_c_bob(wall);
        let lerp = |a: f64, b: f64| a + (b - a) * grown;
        let size = lerp(small, big).round();
        let mid = (lerp(home.0 as f64 + small / 2.0, cx), lerp(home.1 as f64 + small / 2.0, cy + bob));
        let at = ((mid.0 - size / 2.0).round() as i64, (mid.1 - size / 2.0).round() as i64);
        // Down, for a rider turned by `tilt`, is (−sin, cos); the surface
        // runs across it, through the middle, with two waves running along.
        let (sin, cos) = tilt.sin_cos();
        let liquid = move |x: f64, y: f64| {
            let (up, along) = (x * sin - y * cos, x * cos + y * sin);
            Some(if up < dive::wave(along, wall) * size / big { LETTER_LOWER } else { LETTER_UPPER })
        };
        stamp_tinted(c, glyph, at, size as i64, 2, liquid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::glyphs::{LETTER_LOWER, LETTER_UPPER};
    use super::super::looping;
    use std::f64::consts::PI;

    /// Lit pixel rows/columns of the wordmark for `phase` at `frame`.
    fn ink(screen_w: usize, phase: LogoPhase, frame: usize) -> Vec<(i64, i64)> {
        let word = Wordmark::placed(screen_w);
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| {
            word.paint(c, phase, 0.0, dive::Dive::default());
            word.paint_tagline(c, phase);
        }));
        let mut buf = FrameBuffer::new(screen_w, ROWS);
        layers.render(&mut buf, frame);
        let mut px = Vec::new();
        for y in 0..ROWS {
            for x in 0..screen_w {
                let cell = buf.get(x, y);
                let (top, low) = match cell.ch {
                    '▀' => (true, cell.bg.is_some()),
                    '▄' => (cell.bg.is_some(), true),
                    _ => (false, cell.bg.is_some()),
                };
                if top { px.push((x as i64, 2 * y as i64)); }
                if low { px.push((x as i64, 2 * y as i64 + 1)); }
            }
        }
        px
    }

    fn bbox(px: &[(i64, i64)]) -> (i64, i64, i64, i64) {
        let xs = px.iter().map(|p| p.0);
        let ys = px.iter().map(|p| p.1);
        (xs.clone().min().unwrap(), ys.clone().min().unwrap(), xs.max().unwrap(), ys.max().unwrap())
    }

    #[test]
    fn away_leaves_a_big_c_in_the_middle_of_the_loop() {
        let w = 200;
        let (cx, cy) = looping::loop_centre(w);
        for frame in [0, 20, 40] {
            let px = ink(w, LogoPhase::Away(10.0), frame);
            let (x0, y0, x1, y1) = bbox(&px);
            // Only the C is left, BIG_C times its size, in the loop's middle.
            assert!((y1 - y0 + 1 - 8 * BIG_C).abs() <= 4, "{} tall", y1 - y0 + 1);
            assert!((((x0 + x1) as f64 / 2.0) - cx).abs() <= 3.0, "x {x0}..{x1} vs {cx}");
            assert!((((y0 + y1) as f64 / 2.0) - cy).abs() <= 4.0, "y {y0}..{y1} vs {cy}");
            // Clear of the letters riding the loop.
            let reach = px.iter().map(|&(x, y)| ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt()).fold(0.0, f64::max);
            assert!(reach < looping::LOOP_R - 8.0, "reaches {reach}");
        }
    }

    #[test]
    fn away_clears_from_the_end_and_the_c_goes_last() {
        let n = WORD.len();
        // A jumps first, H last of the jumpers...
        let lift = |i: usize, t: f64| away_offset(i, t);
        assert!(lift(n - 1, 0.3) < lift(n - 2, 0.3) && lift(2, 0.3) <= lift(1, 0.3));
        assert_eq!(lift(1, 0.3), 0.0, "H still home");
        // ...and only then does the C leave.
        let h_leaves = (n - 2) as f64 * EXIT_STAGGER;
        assert_eq!(c_grown(LogoPhase::Away(h_leaves)), 0.0);
        assert!(c_grown(LogoPhase::Away(h_leaves + 0.2)) > 0.0);
        assert_eq!(c_grown(LogoPhase::Away(AWAY_DONE)), 1.0);
        for i in 1..n {
            assert!((lift(i, AWAY_DONE) + FALL).abs() < 1e-9, "letter {i} still on screen");
        }
        // The C never jumps: it bobs at home until it glides off.
        let word = Wordmark::placed(200);
        for t in [0.0, C_LEAVES, C_LEAVES + 0.5, AWAY_DONE] {
            assert_eq!(word.bob(0, 3.0, LogoPhase::Away(t)), word.bob(0, 3.0, LogoPhase::Enter(100.0)), "at {t}");
        }
    }

    /// (pink, lavender) pixel counts of the big C above and below its middle
    /// row, turned to `tilt`, at wall time `wall`.
    fn liquid(tilt: f64, wall: f64) -> ((usize, usize), (usize, usize)) {
        let w = 200;
        let word = Wordmark::placed(w);
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| word.paint_big_c(c, &mario('C'), (10, 10), 1.0, wall, tilt)));
        let mut buf = FrameBuffer::new(w, ROWS);
        layers.render(&mut buf, 0);
        let (_, cy) = looping::loop_centre(w);
        let (mut above, mut below) = ((0, 0), (0, 0));
        for y in 0..ROWS * 2 {
            for x in 0..w {
                let cell = buf.get(x, y / 2);
                let color = match cell.ch {
                    '▀' if y % 2 == 0 => Some(cell.color),
                    '▄' if y % 2 == 1 => Some(cell.color),
                    _ => cell.bg,
                };
                let half = if (y as f64) < cy { &mut above } else { &mut below };
                match color {
                    Some(c) if c == LETTER_UPPER => half.0 += 1,
                    Some(c) if c == LETTER_LOWER => half.1 += 1,
                    _ => {}
                }
            }
        }
        (above, below)
    }

    #[test]
    fn liquid_settles_where_gravity_points() {
        // Upright: lavender liquid at the bottom, pink air on top.
        let (above, below) = liquid(0.0, 0.0);
        assert!(above.0 > 3 * above.1 && below.1 > 3 * below.0, "{above:?} {below:?}");
        // Upside down, riding over the top of the loop: the other way round.
        let (above, below) = liquid(PI, 0.0);
        assert!(above.1 > 3 * above.0 && below.0 > 3 * below.1, "{above:?} {below:?}");
    }

    #[test]
    fn liquid_sloshes() {
        let a = liquid(0.0, 0.0);
        assert!((1..30).any(|k| liquid(0.0, k as f64 * 0.1) != a), "surface never moves");
    }

    #[test]
    fn the_c_grows_smoothly() {
        let w = 200;
        // The C alone, frame by frame through the morph.
        let heights: Vec<i64> = (0..=(MORPH_SECS * FPS) as usize)
            .map(|f| {
                let word = Wordmark::placed(w);
                let grown = Easing::EaseInOut.apply(f as f64 / FPS / MORPH_SECS);
                let layers = Layers::new(ROWS).layer(Layer::paint(move |c| word.paint_big_c(c, &mario('C'), (10, 10), grown, 0.0, 0.0)));
                let mut buf = FrameBuffer::new(w, ROWS);
                layers.render(&mut buf, 0);
                let lit = |y: usize| (0..w).any(|x| {
                    let cell = buf.get(x, y / 2);
                    match cell.ch {
                        '▀' => y % 2 == 0 || cell.bg.is_some(),
                        '▄' => y % 2 == 1 || cell.bg.is_some(),
                        _ => cell.bg.is_some(),
                    }
                });
                let rows: Vec<usize> = (0..ROWS * 2).filter(|&y| lit(y)).collect();
                (rows.last().unwrap() - rows[0]) as i64
            })
            .collect();
        assert!(heights.windows(2).all(|p| p[1] >= p[0] && p[1] - p[0] <= 2), "{heights:?}");
        assert!(heights.last().unwrap() - heights[0] >= 8, "{heights:?}");
    }

    #[test]
    fn return_starts_where_away_left_off_and_ends_home() {
        let w = 200;
        assert_eq!(ink(w, LogoPhase::Return(0.0), 7), ink(w, LogoPhase::Away(100.0), 7));
        assert_eq!(ink(w, LogoPhase::Return(10.0), 7), ink(w, LogoPhase::Enter(10.0), 7));
    }

    #[test]
    fn drop_starts_off_screen_and_lands() {
        assert_eq!(drop_offset(0, 0.0), -FALL);
        assert!(drop_offset(10, DROP_START + 10.0 * DROP_STAGGER + DROP_SECS).abs() < 1e-6);
    }

    #[test]
    fn rises_to_the_top_and_stays_on_screen_there() {
        for screen_w in [80, 120, 200, 300] {
            let word = Wordmark::placed(screen_w);
            let bottom = Layout::for_width(screen_w).logo_last();
            let layers = Layers::new(ROWS).layer(Layer::paint(move |c| word.paint(c, LogoPhase::Top, 0.0, dive::Dive::default())));
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
            assert!(highest >= super::super::layout::TOP_MARGIN, "{screen_w}: reaches row {highest}");
            assert!(lowest <= bottom, "{screen_w}: reaches {lowest} > {bottom}");
        }
    }

    #[test]
    fn exit_jumps_every_letter_off_the_top() {
        assert_eq!(exit_offset(0, 0.0), 0.0);
        assert!(exit_offset(0, 0.3) < exit_offset(5, 0.3), "staggered, left first");
        for i in 0..WORD.len() {
            assert!((exit_offset(i, EXIT_DONE) + FALL).abs() < 1e-9);
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
