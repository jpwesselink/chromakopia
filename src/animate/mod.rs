//! Animated terminal effects — framebuffer-based renderer.
//!
//! Effects write `(char, Color)` to a grid. A fixed-rate renderer diffs
//! the grid and flushes only changed cells to stderr.
//!
//! ```no_run
//! # async fn example() {
//! let anim = chromakopia::animate::rainbow("Loading...", 1.0);
//! // ... do async work ...
//! anim.stop();
//! # }
//! ```

pub mod effects;
mod easing;
pub mod framebuffer;
mod scene;

pub use easing::Easing;
pub use effects::{Rainbow, Glow, Plasma, Pulse, Glitch, Radar, Neon, Karaoke, Flap, Scroll, ScrollDirection, Fade, Chain, Composite};
pub use framebuffer::{Cell, Effect, FrameBuffer, AnimationHandle, run_effect, spawn_effect};
pub use scene::{Scene, Line};

use crate::color::Color;
use crate::gradient::Gradient;

/// Compute text dimensions (width, height) from a multiline string.
fn text_dims(text: &str) -> (usize, usize) {
    let lines: Vec<&str> = text.split('\n').collect();
    let height = lines.len();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    (width, height)
}

// ── Standalone animations ──

/// Start a rainbow animation. Speed is a multiplier (1.0 = default).
pub fn rainbow(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Rainbow::new(text), w, h, speed)
}

/// Start a pulse animation (red highlight expanding from center).
pub fn pulse(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Pulse::new(text), w, h, speed)
}

/// Start a glitch animation (random character corruption).
pub fn glitch(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Glitch::new(text), w, h, speed)
}

/// Start a radar animation (spotlight sweep).
pub fn radar(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Radar::new(text), w, h, speed)
}

/// Start a neon animation (flickering bright/dim).
pub fn neon(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Neon::new(text), w, h, speed)
}

/// Start a karaoke animation (progressive highlight).
pub fn karaoke(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Karaoke::new(text), w, h, speed)
}

/// Start a flap animation (split-flap departure board).
pub fn flap(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    let settled = Color::new(0xff, 0xcc, 0x00);
    let flipping = Color::new(0x99, 0x7a, 0x00);
    spawn_effect(Flap::new(text, settled, flipping), w, h, speed)
}

/// Start a glow animation (sweeping gradient spotlight).
pub fn glow(grad: Gradient, text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Glow::new(text, grad.palette(256)), w, h, speed)
}

/// Start a plasma animation (demoscene-style flowing color field).
pub fn plasma(text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Plasma::new(text, crate::presets::storm().palette(256), 0.0), w, h, speed)
}

/// Start a plasma animation with a custom gradient.
pub fn plasma_with(grad: Gradient, text: &str, speed: f64) -> AnimationHandle {
    let (w, h) = text_dims(text);
    spawn_effect(Plasma::new(text, grad.palette(256), 0.0), w, h, speed)
}
