//! The pastel landscape: sky, clouds, hills, ground and cloud shadows —
//! hidden until the `{hills}` code, then built up layer by layer: the sky
//! slides down, the ridges rise from below back to front, the cloud
//! shadows grow in.

use chromakopia::animate::dither;
use chromakopia::prelude::*;
use std::f64::consts::PI;

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
            let a = (PI * x / wl.0 + seed).sin().abs();
            let b = (PI * x / wl.1 + seed * 1.7).sin().abs();
            let top = base - amp * (0.65 * a + 0.35 * b) - lift(x) + dy;
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
pub fn landscape(since: impl Fn() -> Option<f64> + Clone + Send + 'static) -> Layers {
    let h_px = (ROWS * 2) as f64;
    let drop = |i: usize| -> Drop {
        let since = since.clone();
        Box::new(move || since().map(|t| rise(i, t, h_px)))
    };
    // Landscape fills everything below the TOP_PAD sky rows.
    let top = (TOP_PAD * 2) as f64;
    let h = ((ROWS - TOP_PAD) * 2) as f64;
    // Back to front: clouds, greens, then ground.
    let ridges = [
        ridge(top + h * 0.28, 8.0, (23.0, 41.0), 0.3, (CLOUD_LILAC, CLOUD_PINK, 2), drop(0)),
        ridge_lifted(top + h * 0.38, 8.0, (19.0, 37.0), 1.9, (CLOUD_BLUSH, CLOUD_BLUSH, 0), cumulus, drop(1)),
        ridge(top + h * 0.49, 8.0, (21.0, 33.0), 4.1, (MINT_MIST, MINT_PALE, 2), drop(2)),
        ridge(top + h * 0.59, 7.0, (25.0, 43.0), 2.6, (MINT, PISTACHIO, 1), drop(3)),
        ridge(top + h * 0.69, 7.0, (18.0, 31.0), 5.2, (SAGE, MINT, 1), drop(4)),
        ridge(top + h * 0.79, 6.0, (22.0, 39.0), 0.9, (SAGE_DEEP, SAGE, 2), drop(5)),
        ridge(top + h * 0.91, 6.0, (20.0, 35.0), 3.3, (CORAL, PEACH, 1), drop(6)),
        ridge(top + h * 1.03, 5.0, (24.0, 45.0), 6.0, (ROSE, CORAL, 2), drop(7)),
    ];

    // Sky: pastel blue, dithered toward periwinkle in the top rows,
    // sliding down from the top as it comes in.
    let sky_since = since.clone();
    let sky = Layer::paint(move |c| {
        let Some(t) = sky_since() else { return };
        let rows = (c.height as f64 * sky_reveal(t)).round() as usize;
        for y in 0..rows {
            let level = match y {
                0..=1 => 2,
                2..=3 => 1,
                _ => 0,
            };
            for x in 0..c.width {
                c.set(x as i64, y as i64, dither(SKY, SKY_HAZE, level, x, y));
            }
        }
    });

    // Cloud shadows: ovals over the land, one every 64 pixels, each a
    // bit different in size and height — multiplied with NES blue so they
    // cool whatever they pass over. Slower and wider than the ridges, on
    // their own swing. They grow in once the land is there.
    let shadow_since = since;
    let shadows = Layer::paint(move |c| {
        let Some(t) = shadow_since() else { return };
        let grow = shadow_grow(t);
        if grow <= 0.0 {
            return;
        }
        for sx in 0..c.width {
            let x = sx as f64 - c.offset;
            let slot = (x / 64.0).floor();
            let vary = (slot * 2.3).sin(); // -1..1, fixed per oval
            let dx = (x - slot * 64.0 - 32.0) / ((16.0 + 5.0 * vary) * grow);
            if dx.abs() >= 1.0 {
                continue;
            }
            for y in 0..c.height {
                let dy = (y as f64 - top - h * (0.72 + 0.06 * vary)) / (6.0 * grow);
                if dx * dx + dy * dy < 1.0 {
                    c.set(sx as i64, y as i64, SHADOW);
                }
            }
        }
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
    layers.layer(shadows)
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

    #[test]
    fn sky_slides_down_first() {
        assert_eq!(sky_reveal(0.0), 0.0);
        assert!(sky_reveal(0.3) > 0.0 && sky_reveal(0.3) < 1.0);
        assert_eq!(sky_reveal(SKY_SECS), 1.0);
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
