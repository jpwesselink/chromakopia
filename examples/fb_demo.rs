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

     (c) 2026 JP Wesselink — github.com/jpwesselink/chromakopia — MIT License
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');
    let lines: Vec<&str> = text.split('\n').collect();
    let height = lines.len();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let effect = fb_effects::Plasma::new(
        text,
        presets::storm().palette(256),
        rand::random::<f64>() * 1000.0,
    );

    framebuffer::run_effect(effect, width, height, Duration::from_secs(10), 1.0).await;
}
