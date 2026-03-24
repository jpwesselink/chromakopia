/// Framebuffer renderer demo — two-loop architecture.
///
/// Animation task writes to a grid, render task diffs at 25fps.
/// No ANSI string allocation per frame. No parsing.
use chromakopia::animate::fb_effects;
use chromakopia::animate::framebuffer;
use chromakopia::presets;
use std::time::Duration;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|

                  My God, it's full of stars.
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');
    let lines: Vec<&str> = text.split('\n').collect();
    let height = lines.len();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    // Clear screen
    eprint!("\x1B[2J\x1B[H");

    // Native framebuffer sparkle — no ANSI string allocation
    let effect = fb_effects::Sparkle::new(text, presets::starfield().palette(64));
    framebuffer::run_effect(effect, width, height, Duration::from_secs(8), 3.0).await;
}
