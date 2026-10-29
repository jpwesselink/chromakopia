//! The landscape in the docs site's colours: near-black night, plum and
//! dusty-violet fog, sage hills rising to brand green, plum ground.

use chromakopia::animate::dither;
use chromakopia::prelude::*;
use std::f64::consts::PI;

use super::{ROWS, TOP_PAD};

// Cloud shadows multiply with plum.
const SHADOW: Color = Color { r: 0x4E, g: 0x2A, b: 0x4E };

// Palette sampled from the docs site (web/docs/styles/index.css and the
// hero ASCII): #0a0a0a background, plum and dusty-violet ASCII, sage
// greens, brand green #00cc66 / #00ff88. Darkest at the back.
pub const SKY: Color = Color { r: 0x0A, g: 0x0A, b: 0x0A };
const SKY_HAZE: Color = Color { r: 0x1A, g: 0x14, b: 0x20 };
const PLUM: Color = Color { r: 0x4E, g: 0x2A, b: 0x4E };
const PLUM_DEEP: Color = Color { r: 0x2E, g: 0x18, b: 0x2E };
const DUSK: Color = Color { r: 0x54, g: 0x4E, b: 0x6C };
const DUSK_LIGHT: Color = Color { r: 0x6A, g: 0x64, b: 0x86 };
const SAGE_DARK: Color = Color { r: 0x2A, g: 0x3A, b: 0x2A };
const SAGE_DIM: Color = Color { r: 0x48, g: 0x60, b: 0x44 };
const SAGE: Color = Color { r: 0x6C, g: 0x90, b: 0x66 };
const SAGE_LIGHT: Color = Color { r: 0x88, g: 0xBB, b: 0x88 };
// Brand green #00cc66 / #00ff88, washed 35% toward grey.
const BRAND: Color = Color { r: 0x36, g: 0xBA, b: 0x78 };
const BRAND_LIGHT: Color = Color { r: 0x36, g: 0xDC, b: 0x8E };
const GRAPE: Color = Color { r: 0x5A, g: 0x2A, b: 0x54 };
const GRAPE_DEEP: Color = Color { r: 0x2A, g: 0x14, b: 0x28 };
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
        ridge(top + h * 0.28, 8.0, (23.0, 41.0), 0.3, (PLUM_DEEP, PLUM, 2)),
        ridge_lifted(top + h * 0.38, 8.0, (19.0, 37.0), 1.9, (DUSK, DUSK_LIGHT, 1), cumulus),
        ridge(top + h * 0.49, 8.0, (21.0, 33.0), 4.1, (SAGE_DARK, SAGE_DIM, 2)),
        ridge(top + h * 0.59, 7.0, (25.0, 43.0), 2.6, (SAGE_DIM, SAGE, 1)),
        ridge(top + h * 0.69, 7.0, (18.0, 31.0), 5.2, (SAGE, SAGE_LIGHT, 1)),
        ridge(top + h * 0.79, 6.0, (22.0, 39.0), 0.9, (BRAND, BRAND_LIGHT, 1)),
        ridge(top + h * 0.91, 6.0, (20.0, 35.0), 3.3, (PLUM, GRAPE, 1)),
        ridge(top + h * 1.03, 5.0, (24.0, 45.0), 6.0, (GRAPE_DEEP, PLUM, 2)),
    ];

    // Sky: the site's #0a0a0a, dithered toward a faint plum haze up top.
    let sky = Layer::new(|p| {
        let level = match p.y {
            0..=1 => 2,
            2..=3 => 1,
            _ => 0,
        };
        Some(p.dither(SKY, SKY_HAZE, level))
    });

    // Cloud shadows: ovals over the land, one every 64 pixels, each a
    // bit different in size and height — multiplied with plum so they
    // darken whatever they pass over. Slower and wider than the ridges, on
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
