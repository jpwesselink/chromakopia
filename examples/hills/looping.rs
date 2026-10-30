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

/// Radius of the loop, in pixels.
pub const LOOP_R: f64 = 36.0;
/// How far left of where they went in the letters come out, so the way
/// out doesn't run over the way in.
pub const LOOP_SHIFT: f64 = 32.0;
/// Track length the loop adds: once round.
pub const LOOP_EXTRA: f64 = TAU * LOOP_R;
/// Angles a letter can be turned to.
pub const ANGLES: usize = 32;

/// A track shape, picked with `{path …}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shape {
    /// In from the right, once round a vertical loop in the middle of the
    /// screen, out to the left.
    Loop,
}

impl Shape {
    pub fn named(name: &str) -> Option<Shape> {
        match name {
            "loop" => Some(Shape::Loop),
            _ => None,
        }
    }
}

/// Track length `shape` adds over riding straight across a screen `width`
/// wide — what follows waits that much longer.
pub fn extra(shape: Shape, _width: usize) -> f64 {
    match shape {
        Shape::Loop => LOOP_EXTRA,
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
    let phi = into / LOOP_R;
    Spot {
        x: entry - LOOP_R * phi.sin() - LOOP_SHIFT * phi / TAU,
        y: base - LOOP_R + LOOP_R * phi.cos(),
        angle: phi,
    }
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
        let top = track(Shape::Loop, a + PI * LOOP_R, W, BASE);
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
    fn letters_keep_their_spacing_round_the_loop() {
        // One pixel along the track moves a letter about one pixel.
        for k in 0..400 {
            let d = entry_d() - 20.0 + k as f64 * 0.5;
            let (p, q) = (track(Shape::Loop, d, W, BASE), track(Shape::Loop, d + 1.0, W, BASE));
            let step = ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt();
            assert!((0.8..1.3).contains(&step), "d {d}: {step}");
        }
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
    fn nearest_angle_wraps() {
        assert_eq!(angle_index(0.0), 0);
        assert_eq!(angle_index(PI), ANGLES / 2);
        assert_eq!(angle_index(TAU - 0.01), 0);
    }
}
