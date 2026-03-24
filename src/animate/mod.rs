//! Animated terminal effects — framebuffer-based renderer.
//!
//! Effects write `(char, Color)` to a grid. A fixed-rate renderer diffs
//! the grid and flushes only changed cells to stderr.

pub mod effects;
mod easing;
pub mod framebuffer;
mod scene;

pub use easing::Easing;
pub use effects::{Rainbow, Glow, Plasma, Pulse, Glitch, Radar, Neon, Karaoke, Flap, Sparkle};
pub use framebuffer::{Cell, Effect, FrameBuffer, run_effect};
pub use scene::{Scene, Line};
