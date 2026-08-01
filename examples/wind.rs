//! Wind dissolve demo for CPU profiling.
//!
//! Runs for a fixed duration so `samply record cargo run --release --example wind`
//! gets a clean, repeatable workload.

use chromakopia::animate::*;
use chromakopia::{center, gradient, presets, Color};

const LOGO_BLOCK_ART: &str = // do proper ASCII art with a fixed-width font, kids
    "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

const LOGO_ART: &str =
    "         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

const ABOUT_TEXT: &str = "                  a tiny rust crate that turns
                  your terminal into a canvas

              gradients   ·   scenes   ·   plasmas
              particles   ·   ASCII    ·   progress

                composable  ·  zero deps  ·  CI-safe";

#[tokio::main]
async fn main() {
    let fire = gradient(&[
        "#1a1a1a", "#ff69b4", "#00cccc", "#fffacd", "#8b4513", "#1a1a1a",
    ])
    .palette(256);

    let ice = gradient(&[
        // ice blues, with a hint of purple for extra chill
        "#0f2027", "#203a43", "#2c5364", "#5f72be", "#9a8c98", "#0f2027",
    ])
    .palette(256);
    let mist = presets::mist().palette(256);

    let body = format!("{}\n\n{}", LOGO_ART, ABOUT_TEXT);
    let text = center(&body);

    // Single transition: Rainbow → Plasma fire over 3s. After that, Plasma keeps
    // ticking smoothly. No Chain → no frame-counter restart → no stutter.
    let _ = mist; // kept around for tinkering; not used in this arc
    let coloring = Transition::new(
        Plasma::new().palette(ice).seed(31.0),
        Plasma::new().palette(fire).seed(31.0),
        5.0,
        Easing::EaseInOut,
    );

    let wind = Wind::wrap(coloring.on(&text))
        .delay(4.0)
        .duration(3.0)
        .angle_deg(162.0)
        .strength(72.0)
        .flutter(0.7, 0.22, 2.2)
        .damping(31.4)
        .fade_tail(2.0);

    let scene = Scene::new().add(
        FadeEnvelope::new(wind)
            .total(13.0)
            .fade_in(1.0, Easing::EaseOut)
            .fade_out(0.0, Easing::Linear)
            .from_color(Color::new(0, 0, 0)),
    );

    scene.quit_key('q').run(13.0).await;
}
