//! Terminal animation engine.
//!
//! Build animations from **effects** and **scenes**. Effects color text.
//! Scenes stack effects vertically. Three ways to render:
//!
//! - **`.spawn()`** — runs in the background, returns a handle
//! - **`.run(seconds)`** — runs for a fixed duration
//! - **`.frame(n)`** — renders one frame to an ANSI string
//!
//! # Quick start
//!
//! ```no_run
//! # async fn example() {
//! use chromakopia::prelude::*;
//!
//! // One-liner: animate text, fade out after 3 seconds
//! let anim = Rainbow::on("Hello, world!").spawn();
//! tokio::time::sleep(std::time::Duration::from_secs(3)).await;
//! anim.fade_out(1.0);
//! anim.wait().await;
//! # }
//! ```
//!
//! # Effects
//!
//! Color effects are zero-config. Use `.on("text")` to give them text:
//!
//! ```no_run
//! # use chromakopia::prelude::*;
//! // Simple
//! Rainbow::on("hello");
//! Neon::on("blink");
//! Plasma::on("fire").palette(presets::storm().palette(256));
//!
//! // Without text — pure color transform for composition
//! Blend::new(Plasma::new(), Radar::new(), BlendMode::Screen);
//! ```
//!
//! # Scenes
//!
//! Stack multiple effects with [`Scene`]:
//!
//! ```no_run
//! # async fn example() {
//! # use chromakopia::prelude::*;
//! # let white = Color::new(255, 255, 255);
//! Scene::new()
//!     .add(text("MIT License", white))
//!     .blank()
//!     .add(Rainbow::on("colored text"))
//!     .add(Plasma::on("more text").palette(presets::storm().palette(256)))
//!     .run(5.0)
//!     .await;
//! # }
//! ```
//!
//! # Handle
//!
//! Control a running animation:
//!
//! ```no_run
//! # async fn example() {
//! # use chromakopia::prelude::*;
//! let anim = Plasma::on("loading...").spawn();
//! // ...later
//! anim.fade_out(1.0);              // 1 second fade to background
//! anim.fade_out_to(Color::new(0,0,0), 0.5);  // fade to black
//! anim.transition_to(Neon::on("done!"), 1.0); // crossfade
//! anim.wait().await;               // wait for completion
//! # }
//! ```
//!
//! # Inline
//!
//! Render frames yourself for progress bars, spinners, etc:
//!
//! ```no_run
//! # use chromakopia::prelude::*;
//! let effect = Rainbow::on("loading...");
//! for frame in 0..100 {
//!     print!("\r{}", effect.frame(frame));
//! }
//! ```

pub mod effects;
mod easing;
pub mod framebuffer;
mod scene;

pub use easing::Easing;
pub use effects::{Rainbow, Glow, Plasma, Pulse, Glitch, Radar, Neon, Karaoke, Flap, Scroll, ScrollDirection, Spread, SpreadOrigin, Dycp, Fade, FadeEnvelope, Chain, Composite, DelayedStart, Blend, BlendMode, Transition, Solid, text};
pub use framebuffer::{Cell, Effect, EffectExt, On, FrameBuffer, AnimationHandle, run_effect, spawn_effect};
pub use scene::{Scene, Line};
