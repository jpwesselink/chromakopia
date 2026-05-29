mod color;
mod gradient;
mod progress;
pub mod animate;
pub mod presets;

pub use progress::ProgressBar;

#[cfg(not(target_arch = "wasm32"))]
mod terminal;

pub use color::Color;
pub use gradient::{Gradient, HsvSpin, Interpolation};

// ── Color / animation control ────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU8, Ordering};

/// Override state: 0 = auto-detect, 1 = forced on, 2 = forced off.
#[cfg(not(target_arch = "wasm32"))]
static COLOR_OVERRIDE: AtomicU8 = AtomicU8::new(0);

/// Are colors and animations enabled?
///
/// Auto-detects by checking (in order):
/// 1. Manual override via `set_color_enabled()`
/// 2. `NO_COLOR` env var (https://no-color.org) — disables if set
/// 3. `FORCE_COLOR` env var — enables if set
/// 4. Whether stderr is a TTY
#[cfg(not(target_arch = "wasm32"))]
pub fn color_enabled() -> bool {
    match COLOR_OVERRIDE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            // NO_COLOR standard
            if std::env::var_os("NO_COLOR").is_some() { return false; }
            // FORCE_COLOR override
            if std::env::var_os("FORCE_COLOR").is_some() { return true; }
            // TTY check
            use std::io::IsTerminal;
            std::io::stderr().is_terminal()
        }
    }
}

/// Force colors on or off. Pass `None` to return to auto-detection.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_color_enabled(enabled: Option<bool>) {
    COLOR_OVERRIDE.store(match enabled {
        Some(true) => 1,
        Some(false) => 2,
        None => 0,
    }, Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
pub fn color_enabled() -> bool { true }

#[cfg(target_arch = "wasm32")]
pub fn set_color_enabled(_: Option<bool>) {}

#[cfg(not(target_arch = "wasm32"))]
pub use terminal::{bg_color, fg_color, is_dark_theme, is_light_theme, probe_colors, set_bg_color, set_fg_color, terminal_width};

/// Curated imports for animation work.
///
/// ```
/// use chromakopia::prelude::*;
/// ```
pub mod prelude {
    pub use crate::{Color, ProgressBar, color_enabled, set_color_enabled};
    pub use crate::animate::{
        Scene, Line, FrameBuffer, Effect, EffectExt, On,
        Rainbow, Plasma, Glow, Pulse, Radar, Neon, Karaoke, Glitch, Flap,
        Scroll, ScrollDirection, Spread, SpreadOrigin, Dycp, Wind,
        Fade, FadeEnvelope, Chain, Blend, BlendMode, Transition, Composite, DelayedStart,
        Solid, text, Easing, Timeline, AlphaIn, AlphaOut,
    };
    pub use crate::presets;

    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::animate::AnimationHandle;
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::{gradient, bg_color, fg_color, terminal_width};
}

/// Create a gradient from a slice of colors.
///
/// Colors can be hex strings like `"#ff0000"`, CSS-style `"rgb(255,0,0)"`,
/// or named colors like `"red"`.
///
/// ```
/// use chromakopia::gradient;
/// let g = gradient(&["#ff0000", "#00ff00", "#0000ff"]);
/// assert_eq!(g.palette(3).len(), 3);
/// ```
pub fn gradient(colors: &[&str]) -> Gradient {
    let stops: Vec<Color> = colors
        .iter()
        .map(|c| c.parse::<Color>().expect("invalid color"))
        .collect();
    Gradient::new(stops)
}

/// Pad each line with spaces on the right to fill the terminal width.
#[cfg(not(target_arch = "wasm32"))]
pub fn pad(text: &str) -> String {
    let w = terminal::terminal_width();
    text.lines()
        .map(|line| {
            let len = line.chars().count();
            if len < w {
                format!("{}{}", line, " ".repeat(w - len))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Center a text block within the terminal width.
///
/// Uses the widest line to compute a single left-pad, then applies
/// the same offset to every line so multiline text (like figlet banners)
/// stays aligned as a block.
#[cfg(not(target_arch = "wasm32"))]
pub fn center(text: &str) -> String {
    let w = terminal::terminal_width();
    let max_line_width = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    let left = if max_line_width < w { (w - max_line_width) / 2 } else { 0 };
    let pad_left = " ".repeat(left);
    text.lines()
        .map(|line| {
            let len = line.chars().count();
            let right = w.saturating_sub(left + len);
            format!("{}{}{}", pad_left, line, " ".repeat(right))
        })
        .collect::<Vec<_>>()
        .join("\n")
}
