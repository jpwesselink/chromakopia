/// Adapt gradient choice based on detected terminal theme.
///
/// Uses `is_dark_theme()` and `Color::luma()` to pick gradients
/// that contrast well with the terminal background.
use chromakopia::{animate, bg_color, is_dark_theme, presets};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let bg = bg_color();
    let theme = if is_dark_theme() { "dark" } else { "light" };
    eprintln!("Detected {theme} theme (bg #{:02x}{:02x}{:02x}, luma {:.2})",
        bg.r, bg.g, bg.b, bg.luma());

    // Pick a gradient that works on the detected background
    let grad = if is_dark_theme() {
        presets::mist()       // soft light colors pop on dark backgrounds
    } else {
        presets::dark_n_stormy() // dark colors pop on light backgrounds
    };

    let anim = animate::glow(grad, "Theme-adaptive glow", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
