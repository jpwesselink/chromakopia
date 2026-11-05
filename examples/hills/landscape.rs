//! The pastel landscape: sky, clouds, hills, ground and cloud shadows —
//! hidden until the `{hills}` code, then built up layer by layer: the sky
//! slides down, the ridges rise from below back to front, the sky clouds
//! and their shadows grow in.

use chromakopia::animate::dither;
use chromakopia::prelude::*;
use std::f64::consts::PI;

use super::sides::z;
use super::{ROWS, TOP_PAD};

// Cloud shadows multiply with NES blue (2C02 entry 22).
const SHADOW: Color = Color { r: 0x68, g: 0x88, b: 0xFC };

// Pastel landscape, lightest at the back.
pub const SKY: Color = Color { r: 0xA8, g: 0xD8, b: 0xFF };
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
/// The sky's fill as a dither (colour, colour, level) — level 0, just sky —
/// and the ridges', back to front: the cloud banks, the greens, the ground.
pub const SKY_FILL: (Color, Color, u8) = (SKY, SKY_HAZE, 0);
pub const RIDGE_FILLS: [(Color, Color, u8); 8] = [
    (CLOUD_LILAC, CLOUD_PINK, 2),
    (CLOUD_BLUSH, CLOUD_BLUSH, 0),
    (MINT_MIST, MINT_PALE, 2),
    (MINT, PISTACHIO, 1),
    (SAGE, MINT, 1),
    (SAGE_DEEP, SAGE, 2),
    (CORAL, PEACH, 1),
    (ROSE, CORAL, 2),
];
/// Seconds for one full left→right→left swing.
const PERIOD: f64 = 4.5;
/// Peak horizontal displacement of the back layer, in pixels (= cells)
/// either side. Each layer forward swings DEPTH wider than the one behind.
const SWING: f64 = 10.0;
const DEPTH: f64 = 0.18;
/// Phase lag between neighbouring layers, as a fraction of PERIOD.
const LAG: f64 = 0.11;

// Entrance, in seconds since the `{hills}` code.
const SKY_SECS: f64 = 1.2;
/// The clouds wait until the sky is down.
const RISE_START: f64 = SKY_SECS + 0.3;
const RISE_STAGGER: f64 = 0.3;
const RISE_SECS: f64 = 1.0;
const SHADOW_START: f64 = RISE_START + 7.0 * RISE_STAGGER + RISE_SECS;
const SHADOW_SECS: f64 = 0.5;
/// When everything has landed.
pub const ENTRANCE_SECS: f64 = SHADOW_START + SHADOW_SECS;

/// The exit plays the entrance backwards this many times faster.
pub const EXIT_SPEED: f64 = 2.0;
/// Seconds the exit takes.
#[cfg(test)]
pub const EXIT_SECS: f64 = ENTRANCE_SECS / EXIT_SPEED;

/// How much of the sky is down `t` seconds into the entrance (0..=1).
fn sky_reveal(t: f64) -> f64 {
    Easing::EaseOut.apply((t / SKY_SECS).clamp(0.0, 1.0))
}

/// Humps along a ridge line at column `x`, 0..1: two hump widths `wl`
/// mixed for irregular ridges.
fn humps(x: f64, wl: (f64, f64), seed: f64) -> f64 {
    let a = (PI * x / wl.0 + seed).sin().abs();
    let b = (PI * x / wl.1 + seed * 1.7).sin().abs();
    0.65 * a + 0.35 * b
}

/// How far the sky's underside hangs below its straight front, at most.
const SKY_PEAKS: f64 = 8.0;

/// The sky's underside at column `x`, `reveal` (0..=1) of the way down a
/// screen `h` pixels tall: upside-down mountains, peaks pointing down —
/// out of sight above the top at 0, below the bottom at 1.
fn sky_edge(x: f64, reveal: f64, h: f64) -> f64 {
    let front = (h + SKY_PEAKS) * reveal - SKY_PEAKS;
    front + SKY_PEAKS * humps(x, (22.0, 37.0), 2.2)
}

/// Ease-out with a small overshoot past 1 before settling.
fn back_out(p: f64) -> f64 {
    const C1: f64 = 1.70158;
    let q = p - 1.0;
    1.0 + (C1 + 1.0) * q * q * q + C1 * q * q
}

/// How far ridge `i` still sits below its place `t` seconds into the
/// entrance, for a screen `h` pixels tall: `h` before its turn, 0 once
/// landed, briefly negative as it overshoots.
fn rise(i: usize, t: f64, h: f64) -> f64 {
    let p = (t - RISE_START - i as f64 * RISE_STAGGER) / RISE_SECS;
    if p <= 0.0 {
        h
    } else if p >= 1.0 {
        0.0
    } else {
        h * (1.0 - back_out(p))
    }
}

/// How big the cloud shadows have grown (0..=1).
fn shadow_grow(t: f64) -> f64 {
    Easing::EaseOut.apply(((t - SHADOW_START) / SHADOW_SECS).clamp(0.0, 1.0))
}

/// Pixel row where the landscape starts, and how tall it is.
const LAND_TOP: f64 = (TOP_PAD * 2) as f64;
const LAND_H: f64 = ((ROWS - TOP_PAD) * 2) as f64;

/// Highest the cloud banks reach: the front bank's base, less its humps
/// and the tallest cumulus. Sky clouds stay above it.
#[cfg(test)]
fn sky_floor() -> f64 {
    LAND_TOP + LAND_H * 0.38 - 8.0 - 15.0
}

/// Radius of a cloud's puffs, in pixels; puffs sit 1.5 radii apart.
const PUFF_R: f64 = 9.0;

/// How many equal puffs cloud number `slot` has: 1, 2 or 3.
fn puffs_in(slot: f64) -> usize {
    1 + ((slot * 7.3 + 0.4).sin().abs() * 3.0).floor().min(2.0) as usize
}

/// Half the width of a cloud of `n` puffs.
fn cloud_half(n: usize) -> f64 {
    PUFF_R * (1.0 + 0.75 * (n - 1) as f64)
}

/// Cloud number `slot` (one every 64 pixels): where across it `x` is
/// (-1..1), how many puffs it has, and a fixed -1..1 that varies its
/// height. None outside it.
fn cloud_at(x: f64, grow: f64) -> Option<(f64, usize, f64)> {
    if grow <= 0.0 {
        return None;
    }
    let slot = (x / 64.0).floor();
    let n = puffs_in(slot);
    let vary = (slot * 2.3).sin();
    let dx = (x - slot * 64.0 - 32.0) / (cloud_half(n) * grow);
    (dx.abs() < 1.0).then_some((dx, n, vary))
}

/// The cloud at column `x`, high in the open sky, as a smooth shape: its
/// top and flat bottom in pixel rows. A Mario cloud: a row of one, two or
/// three equal round puffs on a flat bottom, as wide as the shadow below.
fn puff_shape(x: f64, grow: f64) -> Option<(f64, f64)> {
    let (dx, n, vary) = cloud_at(x, grow)?;
    let bottom = LAND_TOP + 4.0 + 2.0 * vary;
    // Across the cloud in pixels from its middle; the puffs' centres.
    let px = dx * cloud_half(n) * grow;
    let r = PUFF_R * grow;
    let height = (0..n)
        .map(|i| (i as f64 - (n - 1) as f64 / 2.0) * 1.5 * r)
        .map(|c| (r * r - (px - c).powi(2)).max(0.0).sqrt())
        .fold(0.0, f64::max);
    // A little body under the puffs, so the cloud has a floor.
    Some((bottom - height - 2.0 * grow, bottom))
}

/// `puff_shape` with the front cloud bank's scalloped edge (see
/// `cumulus`): the sky's clouds are bits of our own cloud layer.
fn puff(x: f64, grow: f64) -> Option<(f64, f64)> {
    let (top, bottom) = puff_shape(x, grow)?;
    let scallop = 0.85 + 0.15 * (PI * x / 6.0).sin().abs();
    Some((bottom - (bottom - top) * scallop, bottom))
}

/// The cloud's shadow at column `x`, straight below it on the land: an
/// oval's top and bottom in pixel rows.
fn shadow(x: f64, grow: f64) -> Option<(f64, f64)> {
    let (dx, _, vary) = cloud_at(x, grow)?;
    let centre = LAND_TOP + LAND_H * (0.72 + 0.06 * vary);
    let half = 6.0 * grow * (1.0 - dx * dx).sqrt();
    Some((centre - half, centre + half))
}

/// A hump-ridged layer: everything below the ridge line is filled with
/// a dither of `fill.0`/`fill.1` at level `fill.2`.
/// `base` = valley floor (pixel row), `amp` = hump height, `wl` = two
/// hump widths mixed for irregular ridges.
/// Ridge `i` of the entrance order, sinking `drop()` pixels below its
/// place (None = not drawn at all).
type Drop = Box<dyn Fn() -> Option<f64> + Send>;

fn ridge(base: f64, amp: f64, wl: (f64, f64), seed: f64, fill: (Color, Color, u8), drop: Drop) -> Layer {
    ridge_lifted(base, amp, wl, seed, fill, |_| 0.0, drop)
}

/// `ridge`, with the ridge line raised by `lift(x)` pixels.
fn ridge_lifted(
    base: f64,
    amp: f64,
    wl: (f64, f64),
    seed: f64,
    fill: (Color, Color, u8),
    lift: impl Fn(f64) -> f64 + Send + 'static,
    drop: Drop,
) -> Layer {
    // The ridge line depends only on x: compute it once per column, then
    // fill straight down.
    Layer::paint(move |c| {
        let Some(dy) = drop() else { return };
        for sx in 0..c.width {
            let x = sx as f64 - c.offset;
            let top = base - amp * humps(x, wl, seed) - lift(x) + dy;
            for y in (top.ceil().max(0.0) as usize)..c.height {
                c.set(sx as i64, y as i64, dither(fill.0, fill.1, fill.2, sx, y));
            }
        }
    })
}

/// A towering cumulus every 150 pixels: a dome 15 pixels tall and ~36
/// wide with a scalloped top, lifting the front cloud layer up past the
/// back one into the open sky.
fn cumulus(x: f64) -> f64 {
    let dx = (x.rem_euclid(150.0) - 60.0) / 18.0;
    let dome = (1.0 - dx * dx).max(0.0).sqrt();
    15.0 * dome * (0.8 + 0.2 * (PI * x / 6.0).sin().abs())
}

/// Sky, clouds, hills and cloud shadows, back to front. `since()` is the
/// seconds since the `{hills}` code (None: not yet — nothing is drawn).
#[cfg(test)]
pub fn landscape(since: impl Fn() -> Option<f64> + Clone + Send + 'static) -> Layers {
    landscape_layers(since).into_iter().fold(Layers::new(ROWS), |layers, (_, l)| layers.layer(l))
}

/// `landscape`'s layers, each with its depth (see `sides::z`), so other
/// layers can go in between.
pub fn landscape_layers(since: impl Fn() -> Option<f64> + Clone + Send + 'static) -> Vec<(i32, Layer)> {
    let h_px = (ROWS * 2) as f64;
    let drop = |i: usize| -> Drop {
        let since = since.clone();
        Box::new(move || since().map(|t| rise(i, t, h_px)))
    };
    // Landscape fills everything below the TOP_PAD sky rows.
    let (top, h) = (LAND_TOP, LAND_H);
    // Back to front: clouds, greens, then ground.
    let ridges = [
        ridge(top + h * 0.28, 8.0, (23.0, 41.0), 0.3, RIDGE_FILLS[0], drop(0)),
        ridge_lifted(top + h * 0.38, 8.0, (19.0, 37.0), 1.9, RIDGE_FILLS[1], cumulus, drop(1)),
        ridge(top + h * 0.49, 8.0, (21.0, 33.0), 4.1, RIDGE_FILLS[2], drop(2)),
        ridge(top + h * 0.59, 7.0, (25.0, 43.0), 2.6, RIDGE_FILLS[3], drop(3)),
        ridge(top + h * 0.69, 7.0, (18.0, 31.0), 5.2, RIDGE_FILLS[4], drop(4)),
        ridge(top + h * 0.79, 6.0, (22.0, 39.0), 0.9, RIDGE_FILLS[5], drop(5)),
        ridge(top + h * 0.91, 6.0, (20.0, 35.0), 3.3, RIDGE_FILLS[6], drop(6)),
        ridge(top + h * 1.03, 5.0, (24.0, 45.0), 6.0, RIDGE_FILLS[7], drop(7)),
    ];

    // Sky: pastel blue, dithered toward periwinkle in the top rows,
    // sliding down from the top as it comes in, mountains hanging under it.
    let sky_since = since.clone();
    let sky = Layer::paint(move |c| {
        let Some(t) = sky_since() else { return };
        let reveal = sky_reveal(t);
        for x in 0..c.width {
            let rows = sky_edge(x as f64, reveal, c.height as f64).round().clamp(0.0, c.height as f64) as usize;
            for y in 0..rows {
                let level = match y {
                    0..=1 => 2,
                    2..=3 => 1,
                    _ => 0,
                };
                c.set(x as i64, y as i64, dither(SKY, SKY_HAZE, level, x, y));
            }
        }
    });

    // Sky clouds and their shadows: one every 64 pixels, each a bit
    // different in size and height, the shadow straight below its cloud.
    // Shadows multiply with NES blue so they cool whatever they pass
    // over. Both on one slow, wide swing of their own, so each pair drifts
    // together; they grow in once the land is there.
    let cloud_since = since.clone();
    let clouds = Layer::paint(move |c| {
        let Some(t) = cloud_since() else { return };
        let grow = shadow_grow(t);
        for sx in 0..c.width {
            let Some((top, bottom)) = puff(sx as f64 - c.offset, grow) else { continue };
            let (top, bottom) = (top.round() as i64, bottom.round() as i64);
            // Our cloud layer's colours: the front bank's blush on top,
            // the back bank's lilac-and-pink dither underneath.
            let under = bottom - ((bottom - top) * 2 / 5).max(1);
            for y in top..bottom {
                let fill = if y < under { RIDGE_FILLS[1] } else { RIDGE_FILLS[0] };
                c.set(sx as i64, y, dither(fill.0, fill.1, fill.2, sx, y.max(0) as usize));
            }
        }
    })
    .swing(30.0, 9.0, Easing::EaseInOut)
    .opacity(0.6);
    let shadow_since = since;
    let shadows = Layer::paint(move |c| {
        let Some(t) = shadow_since() else { return };
        let grow = shadow_grow(t);
        for sx in 0..c.width {
            let Some((top, bottom)) = shadow(sx as f64 - c.offset, grow) else { continue };
            for y in top.ceil() as i64..bottom.ceil() as i64 {
                c.set(sx as i64, y, SHADOW);
            }
        }
    })
    .swing(30.0, 9.0, Easing::EaseInOut)
    .blend(BlendMode::Multiply)
    .opacity(0.3);

    let mut layers = vec![(z::SKY, sky), (z::CLOUDS, clouds)];
    for (i, r) in ridges.into_iter().enumerate() {
        let r = r
            .swing(SWING * (1.0 + DEPTH * i as f64), PERIOD, Easing::EaseInOut)
            .phase(-(i as f64) * LAG);
        // The first two ridges are the clouds — slightly see-through.
        let r = if i < 2 { r.opacity(0.85) } else { r };
        layers.push((z::ridge(i), r));
    }
    layers.push((z::SHADOWS, shadows));
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(since: Option<f64>, frame: usize) -> FrameBuffer {
        let layers = landscape(move || since);
        let mut buf = FrameBuffer::new(60, ROWS);
        layers.render(&mut buf, frame);
        buf
    }

    #[test]
    fn nothing_before_the_cue() {
        let buf = render(None, 30);
        assert!((0..ROWS).all(|y| (0..60).all(|x| buf.get(x, y).ch == ' ')));
    }

    #[test]
    fn settles_into_the_full_landscape() {
        // Once everything has landed, the entrance makes no difference.
        assert_eq!(render(Some(ENTRANCE_SECS), 30).to_ansi_string(), render(Some(1000.0), 30).to_ansi_string());
        assert!(render(Some(1000.0), 30).to_ansi_string().contains('▀'));
    }

    /// Pixel (x, y) of a half-block frame.
    fn px(buf: &FrameBuffer, x: usize, y: usize) -> Option<Color> {
        let cell = buf.get(x, y / 2);
        if cell.ch == ' ' {
            return cell.bg;
        }
        if y % 2 == 0 { Some(cell.color) } else { cell.bg }
    }

    #[test]
    fn every_shadow_has_its_cloud_overhead() {
        // Same columns: a cloud casts each shadow, straight down.
        for grow in [0.3, 1.0] {
            for x in -400..400 {
                let x = x as f64 + 0.5;
                assert_eq!(puff(x, grow).is_some(), shadow(x, grow).is_some(), "x {x}, grow {grow}");
            }
        }
        // And there are some.
        assert!((0..400).any(|x| puff(x as f64, 1.0).is_some()));
    }

    #[test]
    fn clouds_hang_in_the_open_sky_above_their_shadows() {
        for x in 0..400 {
            let x = x as f64 + 0.5;
            if let (Some((cloud_top, cloud_bottom)), Some((shadow_top, _))) = (puff(x, 1.0), shadow(x, 1.0)) {
                assert!(cloud_top >= 2.0, "under the top border: {cloud_top}");
                assert!(cloud_top < cloud_bottom);
                assert!(cloud_bottom < sky_floor(), "{cloud_bottom} not above the cloud banks");
                assert!(cloud_bottom < shadow_top);
            }
        }
    }

    /// Each cloud across 0..`width` pixels, as its (top, bottom) per
    /// column (pixel rows).
    fn cloud_tops(width: usize) -> Vec<Vec<(f64, f64)>> {
        let (mut clouds, mut cloud) = (Vec::new(), Vec::new());
        for x in 0..width {
            match puff_shape(x as f64 + 0.5, 1.0) {
                Some(column) => cloud.push(column),
                None if !cloud.is_empty() => clouds.push(std::mem::take(&mut cloud)),
                None => {}
            }
        }
        clouds
    }

    /// The tops of a cloud's puffs: where its outline peaks.
    fn peaks(cloud: &[(f64, f64)]) -> Vec<f64> {
        cloud.windows(3).filter(|w| w[1].0 < w[0].0 && w[1].0 <= w[2].0).map(|w| w[1].0).collect()
    }

    #[test]
    fn clouds_are_rows_of_equal_puffs() {
        // Mario clouds: one, two or three round puffs, all as tall.
        for cloud in cloud_tops(1200) {
            let p = peaks(&cloud);
            assert!((1..=3).contains(&p.len()), "{} puffs: {cloud:?}", p.len());
            let (lo, hi) = p.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            assert!(hi - lo < 0.5, "puffs not alike: {p:?}");
            // Properly round: the puffs stand well up off the flat bottom.
            assert!(cloud[0].1 - lo >= 6.0, "flat: {cloud:?}");
            assert!(cloud.iter().all(|c| c.1 == cloud[0].1), "bottom not flat");
        }
    }

    #[test]
    fn clouds_have_our_cloud_banks_scalloped_edge() {
        // The edge dips a little every few pixels, never more than a fifth.
        let dips = (0..1200)
            .filter_map(|x| Some((puff(x as f64 + 0.5, 1.0)?, puff_shape(x as f64 + 0.5, 1.0)?)))
            .map(|((top, bottom), (smooth, _))| (top - smooth) / (bottom - smooth))
            .collect::<Vec<f64>>();
        assert!(dips.iter().all(|&d| (-1e-9..=0.16).contains(&d)));
        assert!(dips.iter().any(|&d| d > 0.1));
    }

    #[test]
    fn clouds_come_in_ones_twos_and_threes() {
        let counts: std::collections::BTreeSet<usize> = cloud_tops(2400).iter().map(|t| peaks(t).len()).collect();
        assert_eq!(counts, [1, 2, 3].into_iter().collect(), "{counts:?}");
    }

    #[test]
    fn clouds_grow_in_with_their_shadows() {
        assert!((0..400).all(|x| puff(x as f64, 0.0).is_none()));
        let width = |grow| (0..400).filter(|&x| puff(x as f64, grow).is_some()).count();
        assert!(width(0.5) < width(1.0));
        // On screen: something not sky in the open sky once they're in.
        let sky_rows = 2..sky_floor() as usize;
        // (The sky dithers between exactly SKY and SKY_HAZE.)
        let sky = |c: Color| c == SKY || c == SKY_HAZE;
        let full = render(Some(1000.0), 0);
        assert!(sky_rows.clone().any(|y| (0..60).any(|x| px(&full, x, y).is_some_and(|c| !sky(c)))));
        let early = render(Some(SHADOW_START - 0.1), 0);
        assert!(sky_rows.clone().all(|y| (0..60).all(|x| px(&early, x, y).is_none_or(sky))));
    }

    #[test]
    fn sky_slides_down_first() {
        assert_eq!(sky_reveal(0.0), 0.0);
        assert!(sky_reveal(0.3) > 0.0 && sky_reveal(0.3) < 1.0);
        assert_eq!(sky_reveal(SKY_SECS), 1.0);
    }

    #[test]
    fn sky_comes_down_with_upside_down_mountains_under_it() {
        let h = 100.0;
        // Hidden before, all of the screen after.
        assert!((0..400).all(|x| sky_edge(x as f64, 0.0, h) <= 0.0));
        assert!((0..400).all(|x| sky_edge(x as f64, 1.0, h) >= h));
        // On the way down its underside hangs in peaks, not a straight line.
        let edge: Vec<f64> = (0..400).map(|x| sky_edge(x as f64, 0.5, h)).collect();
        let (lo, hi) = edge.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &e| (lo.min(e), hi.max(e)));
        assert!(hi - lo > 4.0, "flat underside: {lo}..{hi}");
        let peaks = edge.windows(3).filter(|w| w[1] > w[0] && w[1] >= w[2]).count();
        assert!(peaks >= 10, "{peaks} peaks");
        // Rendered: some column has sky lower down than its neighbour.
        let mid = render(Some(SKY_SECS * 0.3), 0);
        let depth = |x| (0..ROWS * 2).take_while(|&y| px(&mid, x, y).is_some()).count();
        assert!((0..59).any(|x| depth(x) != depth(x + 1)));
    }

    #[test]
    fn clouds_wait_for_the_sky() {
        assert_eq!(rise(0, SKY_SECS, 100.0), 100.0);
        assert!(rise(0, SKY_SECS + 0.5, 100.0) < 100.0);
    }

    #[test]
    fn layers_rise_back_to_front_with_a_little_bounce() {
        let h = 100.0;
        // Before its turn a layer is fully below the screen.
        assert_eq!(rise(3, 0.0, h), h);
        // Mid-entrance the back layer has landed while the front still rises.
        let t = RISE_START + 3.0 * RISE_STAGGER + RISE_SECS;
        assert!(rise(0, t, h).abs() < 1e-9);
        assert!(rise(7, t, h) > 0.0);
        // It overshoots above its place on the way in.
        let lowest = (0..100).map(|k| rise(0, RISE_START + RISE_SECS * k as f64 / 100.0, h)).fold(f64::MAX, f64::min);
        assert!(lowest < 0.0, "no bounce: {lowest}");
    }
}
