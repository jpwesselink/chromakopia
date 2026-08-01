//! Wind dissolve, diagonal version: tear starts at the BOTTOM-LEFT corner
//! and sweeps up-and-to-the-right; particles fly off toward the upper-right.
//!
//! Run as: `cargo run --release --example wind_diagonal`
//!
//! The key knobs are `.angle_deg(...)`:
//!   - 315° → wind blows toward upper-right (this demo)
//!   - 225° → wind blows toward upper-left (mirror image)
//!   - 45°  → toward lower-right (dust drifts down off-screen)
//!   - 135° → toward lower-left
//! Wind angles use standard math convention (0° = right, CCW) with screen-y
//! growing downward.

use chromakopia::animate::*;
use chromakopia::{center, gradient, Color};

const BANNER: &str = "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

#[tokio::main]
async fn main() {
    let fire = gradient(&[
        "#1a1a1a", "#ff69b4", "#00cccc", "#fffacd", "#8b4513", "#1a1a1a",
    ])
    .palette(256);

    // Three vertically stacked copies of the banner (blank row between).
    let banner3 = format!("{0}\n\n{0}\n\n{0}", BANNER);
    let text = center(&banner3);
    let plasma_text = Plasma::new().palette(fire).seed(17.0).on(&text);

    let wind = Wind::wrap(plasma_text)
        .delay(2.0)           // hold the text briefly so the viewer reads it
        .duration(5.0)        // how long the whole dissolve takes
        .angle_deg(315.0)     // wind toward upper-right (tear from bottom-left)
        .strength(48.0)       // terminal velocity in cells/s
        .flutter(0.0, 0.0, 0.0) // no cloth-flutter pre-tear
        .lean(0.0)            // no pre-release drift — at terminal-cell resolution it looks like teleporting, not motion
        .damping(1.2)
        .fade_tail(1.0);

    // Pre-pad with 1.5s of blank/black before the fade-in starts.
    let staged = DelayedStart::new(
        1.5,
        FadeEnvelope::new(wind)
            .total(9.0)
            .fade_in(1.5, Easing::EaseOut)
            .fade_out(0.0, Easing::Linear)
            .from_color(Color::new(0, 0, 0)),
    );

    let scene = Scene::new().add(staged);
    scene.quit_key('q').run(11.0).await;
}
