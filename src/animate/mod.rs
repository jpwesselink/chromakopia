//! Animated terminal effects — framebuffer-based renderer.
//!
//! Effects write `(char, Color)` to a grid. A fixed-rate renderer diffs
//! the grid and flushes only changed cells to stderr.

mod effects;
mod easing;
pub mod framebuffer;
mod scene;

pub use easing::Easing;
pub use framebuffer::{Cell, Effect, FrameBuffer, run_effect};
