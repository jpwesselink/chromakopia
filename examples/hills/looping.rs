//! Paths for the scroll text: between `{path loop}` (or another shape)
//! and `{path off}` the letters leave the scroller's line and ride a track
//! across the screen — each letter turning with it, upside down over the
//! top of a loop.
//!
//! Letters are turned the way the old demos did it: every angle worked out
//! once up front (supersampled, so thin strokes survive), then just looked
//! up per frame.

use chromakopia::prelude::*;
use std::f64::consts::TAU;

use super::glyphs::{LETTER_HIGHLIGHT, LETTER_LOWER, LETTER_UPPER};
use super::script::Glyph;
use super::scroller;

/// Radius of the loop, in pixels.
pub const LOOP_R: f64 = 36.0;
/// How far left of where they went in the letters come out, so the way
/// out doesn't run over the way in.
pub const LOOP_SHIFT: f64 = 32.0;
/// How much further apart letters stand in the loop than on the straight:
/// they speed up going round, so turned letters stand free.
pub const LOOP_SPREAD: f64 = 1.4;
/// Text the loop takes, in pixels: once round, at the loop's speed.
pub const LOOP_EXTRA: f64 = TAU * LOOP_R / LOOP_SPREAD;
/// Track pixels over which the scroller's wave dies down before the loop
/// (and swells back after it).
pub const WAVE_FADE: f64 = 24.0;
/// Angles a letter can be turned to.
pub const ANGLES: usize = 32;

/// A track shape, picked with `{path …}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shape {
    /// In from the right, once round a vertical loop in the middle of the
    /// screen, out to the left.
    Loop,
    /// Riding the surface of the big C's liquid, turning with it.
    Spin,
}

impl Shape {
    pub fn named(name: &str) -> Option<Shape> {
        match name {
            "loop" => Some(Shape::Loop),
            "spin" => Some(Shape::Spin),
            _ => None,
        }
    }
}

/// Track length `shape` adds over riding straight across a screen `width`
/// wide — what follows waits that much longer.
pub fn extra(shape: Shape, _width: usize) -> f64 {
    match shape {
        Shape::Loop => LOOP_EXTRA,
        // What follows waits for the spin to come to rest instead.
        Shape::Spin => 0.0,
    }
}

/// How much of the scroller's wave a letter `d` pixels along `shape`'s
/// track rides (0..=1): all of it on the straight, dying down to none as
/// it nears the loop, swelling back once out — never a jump.
pub fn wave_weight(shape: Shape, d: f64, width: usize) -> f64 {
    match shape {
        Shape::Loop => {
            let (start, end) = (loop_start(width), loop_start(width) + LOOP_EXTRA);
            let away = if d < start { start - d } else if d > end { d - end } else { 0.0 };
            (away / WAVE_FADE).min(1.0)
        }
        // It rides the liquid's slosh instead (see `on_surface`).
        Shape::Spin => 0.0,
    }
}

/// Where a letter is on the track: its baseline middle, and how far it has
/// turned (radians, clockwise on screen; 0 = upright).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
}

/// Screen x where the loop starts: half a shift right of the middle, so
/// the loop — which drifts left by the shift on the way round — turns about
/// the middle.
fn entry_x(width: usize) -> f64 {
    width as f64 / 2.0 + LOOP_SHIFT / 2.0
}

/// Track distance from the right edge to the start of the loop.
pub fn loop_start(width: usize) -> f64 {
    width as f64 - entry_x(width)
}

/// The spot `d` pixels along `shape`'s track from the right edge of a
/// screen `width` wide, starting and ending on row `base`.
pub fn track(shape: Shape, d: f64, width: usize, base: f64) -> Spot {
    match shape {
        Shape::Loop => loop_track(d, width, base),
        // Level through the big C's middle (`on_surface` turns and sloshes it).
        Shape::Spin => Spot { x: width as f64 - d, y: loop_centre(width).1, angle: 0.0 },
    }
}

/// The loop's track: the straight parts on row `base`, round the loop in
/// the middle of the screen.
fn loop_track(d: f64, width: usize, base: f64) -> Spot {
    let entry = entry_x(width);
    let into = d - loop_start(width);
    if into < 0.0 {
        return Spot { x: width as f64 - d, y: base, angle: 0.0 };
    }
    if into >= LOOP_EXTRA {
        return Spot { x: entry - LOOP_SHIFT - (into - LOOP_EXTRA), y: base, angle: 0.0 };
    }
    // Left along the bottom, up, back right over the top, down — drifting
    // left by the shift on the way round.
    let phi = into * LOOP_SPREAD / LOOP_R;
    Spot {
        x: entry - LOOP_R * phi.sin() - LOOP_SHIFT * phi / TAU,
        y: base - LOOP_R + LOOP_R * phi.cos(),
        angle: phi,
    }
}

/// The middle of the loop on a screen `width` wide, in pixels.
pub fn loop_centre(width: usize) -> (f64, f64) {
    // The loop drifts left by the shift as it goes round, from half a
    // shift right of the middle: it turns about the middle.
    (width as f64 / 2.0, (super::SCROLLER_Y + super::scroller::BASE) as f64 - LOOP_R)
}

/// Scrolled pixels per turn of the big C's liquid — and of the letters
/// riding its surface with `{path spin}`.
pub const SPIN_TURN: f64 = 200.0;
/// Scrolled pixels the liquid takes to come up to speed before the riders
/// (half a turn), and to come to rest (another half).
pub const SPIN_UP: f64 = SPIN_TURN;
pub const SPIN_DOWN: f64 = SPIN_TURN;
/// Scrolled pixels from when the liquid starts turning until the first
/// rider comes in: the spin-up, then on its own at full speed until it's
/// level again — so the riders come in along a level surface.
pub const SPIN_LEAD: f64 = SPIN_TURN + SPIN_UP / 2.0;

/// How far from the big C's middle a rider on its surface can still be on
/// a screen `width` wide: past the farthest corner, plus a letter.
fn spin_reach(width: usize) -> f64 {
    let (cx, cy) = loop_centre(width);
    let h = (super::ROWS * 2) as f64;
    cx.max(width as f64 - cx).hypot(cy.max(h - cy)) + 16.0
}

/// The spin's ride on a screen `width` wide: how far the text has
/// scrolled when the first `{path spin}` letter comes in at the right
/// edge, and when the last one is out of reach. None when nothing spins.
pub fn ride(glyphs: &[Glyph], width: usize) -> Option<(f64, f64)> {
    let mut riders = glyphs.iter().filter(|g| g.path == Some(Shape::Spin));
    let mid = |g: &Glyph| g.x + scroller::advance(g.font, g.ch) as f64 / 2.0;
    let first = riders.next()?;
    let last = riders.next_back().unwrap_or(first);
    let (cx, _) = loop_centre(width);
    Some((mid(first), mid(last) + width as f64 - cx + spin_reach(width)))
}

/// The liquid's turn for a `ride`, in scrolled pixels: when it starts
/// turning, when it starts slowing down, and when it's at rest again.
fn spin((start, end): (f64, f64)) -> (f64, f64, f64) {
    // As many turns at full speed with the riders as fit while they're on
    // (at least one), then slowing down — which turns it as far as half
    // the slow-down at full speed would, so it lands level.
    let laps = ((end - start - SPIN_DOWN / 2.0) / SPIN_TURN).floor().max(1.0);
    let slow = start + laps * SPIN_TURN - SPIN_DOWN / 2.0;
    (start - SPIN_LEAD, slow, slow + SPIN_DOWN)
}

/// When the liquid is at rest again after `ride`, in scrolled pixels.
pub fn spin_done(ride: (f64, f64)) -> f64 {
    spin(ride).2
}

/// Which way is down in the big C, `scrolled` pixels in (radians,
/// clockwise): it gets going before the riders come in, is level as the
/// first arrives, turns with them a lap per SPIN_TURN, then slows down and
/// lands level. 0 otherwise.
pub fn tilt(scrolled: f64, ride: Option<(f64, f64)>) -> f64 {
    let Some(ride) = ride else { return 0.0 };
    let (from, slow, rest) = spin(ride);
    if scrolled < from || scrolled >= rest {
        return 0.0;
    }
    // Radians per scrolled pixel at full speed. The speed ramps along a
    // smoothstep, so a ramp turns it half as far as full speed would.
    let speed = TAU / SPIN_TURN;
    let start = ride.0;
    let turned = if scrolled < from + SPIN_UP {
        let q = (scrolled - from) / SPIN_UP;
        speed * (SPIN_UP * (q.powi(3) - q.powi(4) / 2.0) - SPIN_LEAD + SPIN_UP / 2.0)
    } else if scrolled < slow {
        speed * (scrolled - start)
    } else {
        let q = (scrolled - slow) / SPIN_DOWN;
        speed * (slow - start + SPIN_DOWN * (q - q.powi(3) + q.powi(4) / 2.0))
    };
    turned.rem_euclid(TAU)
}

/// Where a letter riding the liquid's surface is, from the big C's middle:
/// `along` pixels along the surface (as on the straight: + is right when
/// level), with the surface turned to `tilt` and sloshing, `wall` seconds
/// in. Its baseline middle, and its angle — the surface's turn plus its
/// slope under the letter, `width` pixels wide.
pub fn on_surface(along: f64, tilt: f64, wall: f64, width: f64) -> Spot {
    let h = |a: f64| super::dive::wave(a, wall);
    let up = h(along);
    let slope = (h(along + width / 2.0) - h(along - width / 2.0)) / width;
    let (sin, cos) = tilt.sin_cos();
    // Along runs (cos, sin) on screen, up runs (sin, −cos).
    Spot { x: along * cos + up * sin, y: along * sin - up * cos, angle: tilt - slope.atan() }
}

/// The nearest of the `ANGLES` turns to `angle`.
pub fn angle_index(angle: f64) -> usize {
    (angle.rem_euclid(TAU) / TAU * ANGLES as f64).round() as usize % ANGLES
}

/// An 8×8 glyph at `scale`×, turned by angle `index` about its middle:
/// its lit pixels as (x, y, face colour) around (0, 0) — the pixel right
/// and below the middle is (0, 0). Each pixel samples the glyph 4×4 times
/// and lights when over 40% of it is ink, so strokes stay whole at odd
/// angles. Face colours stay lit from above on screen whatever the angle,
/// like the upright letters: cream along each top edge, pink upper half,
/// lavender lower.
pub fn turned(glyph: &[u8; 8], scale: i64, index: usize) -> Vec<(i64, i64, Color)> {
    const SUB: usize = 4;
    let n = 8 * scale;
    let lit = |gx: i64, gy: i64| (0..8).contains(&gy) && (0..8).contains(&gx) && glyph[gy as usize] & (0x80 >> gx) != 0;
    let ink = |u: f64, v: f64| u >= 0.0 && v >= 0.0 && lit(u as i64 / scale, v as i64 / scale);
    let angle = TAU * index as f64 / ANGLES as f64;
    let (sin, cos) = angle.sin_cos();
    let half = n as f64 / 2.0;
    // Back from screen to glyph: turn the other way, about the middle.
    let back = |px: f64, py: f64| (cos * px + sin * py + half, -sin * px + cos * py + half);
    let reach = (half * std::f64::consts::SQRT_2).ceil() as i64 + 1;
    let mut ink_at = std::collections::HashSet::new();
    for y in -reach..reach {
        for x in -reach..reach {
            let hits = (0..SUB * SUB)
                .filter(|k| {
                    let (sx, sy) = ((k % SUB) as f64 + 0.5, (k / SUB) as f64 + 0.5);
                    let (u, v) = back(x as f64 + sx / SUB as f64, y as f64 + sy / SUB as f64);
                    ink(u, v)
                })
                .count();
            if hits * 5 > SUB * SUB * 2 {
                ink_at.insert((x, y));
            }
        }
    }
    // Shaded on screen, not on the letter: the light stays put as it turns.
    let mut face: Vec<_> = ink_at
        .iter()
        .map(|&(x, y)| {
            let color = if !ink_at.contains(&(x, y - 1)) {
                LETTER_HIGHLIGHT
            } else if y < 0 {
                LETTER_UPPER
            } else {
                LETTER_LOWER
            };
            (x, y, color)
        })
        .collect();
    face.sort_by_key(|&(x, y, _)| (y, x));
    face
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    const W: usize = 200;
    const BASE: f64 = 80.0;

    /// Track distance where the loop starts.
    fn entry_d() -> f64 {
        loop_start(W)
    }

    #[test]
    fn loop_starts_where_the_turning_starts() {
        assert_eq!(track(Shape::Loop, loop_start(W) - 0.5, W, BASE).angle, 0.0);
        assert!(track(Shape::Loop, loop_start(W) + 0.5, W, BASE).angle > 0.0);
    }

    #[test]
    fn loops_round_the_middle_of_the_screen() {
        let a = entry_d();
        let xs: Vec<f64> = (0..1000).map(|k| track(Shape::Loop, a + LOOP_EXTRA * k as f64 / 1000.0, W, BASE).x).collect();
        let (lo, hi) = xs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(((lo + hi) / 2.0 - W as f64 / 2.0).abs() < 2.0, "loop spans {lo}..{hi}");
    }

    #[test]
    fn straight_until_the_loop() {
        for d in [0.0, 10.0, entry_d() - 1.0] {
            assert_eq!(track(Shape::Loop, d, W, BASE), Spot { x: W as f64 - d, y: BASE, angle: 0.0 });
        }
    }

    #[test]
    fn goes_round_once_upside_down_over_the_top() {
        let a = entry_d();
        let top = track(Shape::Loop, a + PI * LOOP_R / LOOP_SPREAD, W, BASE);
        assert!((top.y - (BASE - 2.0 * LOOP_R)).abs() < 1e-9, "{top:?}");
        assert!((top.angle - PI).abs() < 1e-9);
        let lowest = (0..=1000).map(|k| track(Shape::Loop, a + TAU * LOOP_R * k as f64 / 1000.0, W, BASE).y).fold(f64::MIN, f64::max);
        assert!(lowest <= BASE + 1e-9, "never dips below the track");
    }

    #[test]
    fn comes_out_further_right_by_the_ride_less_the_shift() {
        let d = entry_d() + LOOP_EXTRA + 10.0;
        let out = track(Shape::Loop, d, W, BASE);
        assert!((out.x - (W as f64 - d + LOOP_EXTRA - LOOP_SHIFT)).abs() < 1e-9, "{out:?}");
        assert_eq!((out.y, out.angle), (BASE, 0.0));
    }

    #[test]
    fn letters_spread_out_in_the_loop_only() {
        // On the straight, a pixel of text is a pixel of track; in the loop
        // letters speed up, standing LOOP_SPREAD times further apart.
        let step = |d: f64| {
            let (p, q) = (track(Shape::Loop, d, W, BASE), track(Shape::Loop, d + 1.0, W, BASE));
            ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt()
        };
        let (a, ride) = (entry_d(), LOOP_EXTRA);
        for d in [a - 30.0, a - 2.0, a + ride + 1.0, a + ride + 30.0] {
            assert!((step(d) - 1.0).abs() < 1e-9, "straight at {d}: {}", step(d));
        }
        for k in 1..100 {
            let d = a + ride * k as f64 / 100.0 - 0.5;
            let s = step(d);
            assert!((0.85 * LOOP_SPREAD..1.25 * LOOP_SPREAD).contains(&s), "loop at {d}: {s}");
        }
        assert!(LOOP_SPREAD >= 1.3, "far enough apart to stand free");
    }

    /// The pixel set of `face`.
    fn pixels(face: &[(i64, i64, Color)]) -> Vec<(i64, i64)> {
        let mut px: Vec<_> = face.iter().map(|&(x, y, _)| (x, y)).collect();
        px.sort();
        px
    }

    const W_GLYPH: [u8; 8] = [0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00];

    #[test]
    fn upright_is_the_letter_as_drawn() {
        // Turned by 0: every lit pixel of the 2× glyph, centred on (0, 0).
        let face = turned(&W_GLYPH, 2, 0);
        let mut want = Vec::new();
        for gy in 0..16i64 {
            for gx in 0..16i64 {
                if W_GLYPH[(gy / 2) as usize] & (0x80 >> (gx / 2)) != 0 {
                    want.push((gx - 8, gy - 8));
                }
            }
        }
        want.sort();
        assert_eq!(pixels(&face), want);
    }

    #[test]
    fn light_stays_put_as_letters_turn() {
        // Lit from above on screen at every angle, like the upright letters:
        // cream wherever nothing is directly above, pink in the upper half,
        // lavender in the lower — however the letter is turned.
        for i in 0..ANGLES {
            let face = turned(&W_GLYPH, 2, i);
            let lit: std::collections::HashSet<(i64, i64)> = face.iter().map(|&(x, y, _)| (x, y)).collect();
            for &(x, y, color) in &face {
                let want = if !lit.contains(&(x, y - 1)) {
                    LETTER_HIGHLIGHT
                } else if y < 0 {
                    LETTER_UPPER
                } else {
                    LETTER_LOWER
                };
                assert_eq!(color, want, "angle {i} at ({x}, {y})");
            }
        }
    }

    #[test]
    fn quarter_turns_lose_nothing() {
        let up = pixels(&turned(&W_GLYPH, 2, 0));
        // A quarter turn clockwise on screen: (x, y) → (-y - 1, x).
        let mut quarter: Vec<_> = up.iter().map(|&(x, y)| (-y - 1, x)).collect();
        quarter.sort();
        assert_eq!(pixels(&turned(&W_GLYPH, 2, ANGLES / 4)), quarter);
        let mut half: Vec<_> = up.iter().map(|&(x, y)| (-x - 1, -y - 1)).collect();
        half.sort();
        assert_eq!(pixels(&turned(&W_GLYPH, 2, ANGLES / 2)), half);
    }

    #[test]
    fn odd_angles_keep_most_of_the_ink() {
        let ink = turned(&W_GLYPH, 2, 0).len() as f64;
        for i in 1..ANGLES {
            let n = turned(&W_GLYPH, 2, i).len() as f64;
            assert!((0.75 * ink..1.25 * ink).contains(&n), "angle {i}: {n} vs {ink}");
        }
    }

    #[test]
    fn wave_dies_down_into_the_loop_and_back_up_after() {
        let (a, end) = (entry_d(), entry_d() + LOOP_EXTRA);
        assert_eq!(wave_weight(Shape::Loop, a - 100.0, W), 1.0);
        assert_eq!(wave_weight(Shape::Loop, a, W), 0.0);
        assert_eq!(wave_weight(Shape::Loop, a + LOOP_EXTRA / 2.0, W), 0.0);
        assert_eq!(wave_weight(Shape::Loop, end + 100.0, W), 1.0);
        // Gently, not in a jump.
        let w = |d| wave_weight(Shape::Loop, d, W);
        assert!(w(a - 10.0) > 0.0 && w(a - 10.0) < 1.0);
        assert!(w(end + 10.0) > 0.0 && w(end + 10.0) < 1.0);
        for k in 0..400 {
            let d = a - 50.0 + k as f64;
            assert!((w(d + 1.0) - w(d)).abs() <= 1.0 / WAVE_FADE + 1e-9, "at {d}");
        }
    }

    #[test]
    fn tilt_turns_like_a_rider_while_letters_loop() {
        let ride = (1000.0, 1000.0 + 2.9 * SPIN_TURN);
        let (from, rest) = (1000.0 - SPIN_LEAD, spin_done(ride));
        assert_eq!(tilt(from - 1.0, Some(ride)), 0.0, "level before");
        assert_eq!(tilt(500.0, None), 0.0, "no spin, no tilt");
        // Already turning before the first rider comes in, level as it does,
        // then a turn per SPIN_TURN.
        assert!(tilt(1000.0 - SPIN_TURN / 4.0, Some(ride)) > 0.0);
        for k in 0..=5 {
            let s = 1000.0 + k as f64 * SPIN_TURN / 4.0;
            let want = (TAU * (s - 1000.0) / SPIN_TURN).rem_euclid(TAU);
            let d = (tilt(s, Some(ride)) - want).rem_euclid(TAU);
            assert!(d.min(TAU - d) < 1e-9, "at {s}");
        }
        // Two turns fit before slowing down, landing level with the riders still on.
        assert!((rest - (1000.0 + 2.0 * SPIN_TURN + SPIN_DOWN / 2.0)).abs() < 1e-9, "{rest}");
        assert!(rest <= ride.1);
        assert_eq!(tilt(rest + 1.0, Some(ride)), 0.0);
        // A short ride still gets one turn.
        assert!(spin_done((0.0, 10.0)) >= SPIN_TURN);
        // Smooth all the way: no jumps, getting going and slowing down gently.
        let step = |s: f64| {
            let d = (tilt(s + 1.0, Some(ride)) - tilt(s, Some(ride))).rem_euclid(TAU);
            d.min(TAU - d)
        };
        let full = TAU / SPIN_TURN;
        for k in 0..(rest - from) as usize + 4 {
            let s = from - 2.0 + k as f64;
            assert!(step(s) <= full + 1e-6, "jump at {s}: {}", step(s));
        }
        assert!(step(from + 1.0) < full / 50.0 && step(rest - 2.0) < full / 50.0);
    }

    #[test]
    fn nearest_angle_wraps() {
        assert_eq!(angle_index(0.0), 0);
        assert_eq!(angle_index(PI), ANGLES / 2);
        assert_eq!(angle_index(TAU - 0.01), 0);
    }
}
