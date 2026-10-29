//! The pastel landscape: sky, clouds, hills, ground and cloud shadows.

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

/// A towering cumulus every 150 pixels: a dome 15 pixels tall and ~36
/// wide with a scalloped top, lifting the front cloud layer up past the
/// back one into the open sky.
fn cumulus(x: f64) -> f64 {
    let dx = (x.rem_euclid(150.0) - 60.0) / 18.0;
    let dome = (1.0 - dx * dx).max(0.0).sqrt();
    15.0 * dome * (0.8 + 0.2 * (PI * x / 6.0).sin().abs())
}

/// Sky, clouds, hills and cloud shadows, back to front.
pub fn landscape() -> Layers {
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
        (dx * dx + dy * dy < 1.0).then_some(SHADOW)
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
