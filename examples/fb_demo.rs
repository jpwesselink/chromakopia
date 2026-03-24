/// Framebuffer renderer demo — mixed static and animated rows.
///
/// Static rows are written once and never re-emitted (zero cost).
/// Animated rows get plasma every frame. Diff renderer skips static rows.
use chromakopia::animate::fb_effects::{LayeredEffect, RowPlasma, StaticText};
use chromakopia::animate::framebuffer;
use chromakopia::presets;
use chromakopia::Color;
use std::time::Duration;

const BANNER: &str = r#"   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|"#;

#[tokio::main]
async fn main() {
    let line1 = "(c) 2026 JP Wesselink";
    let line2 = "github.com/jpwesselink/chromakopia";
    let banner_lines: Vec<&str> = BANNER.lines().collect();

    // Layout:
    // row 0: static — credit line
    // row 1: empty
    // row 2-6: animated — figlet banner with plasma
    // row 7: empty
    // row 8: static — github url

    let height = 9;
    let width = banner_lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

    let seed: f64 = rand::random::<f64>() * 1000.0;

    let static_text = StaticText::new(vec![
        (0, line1, Color::new(100, 100, 100)),
        (8, line2, Color::new(100, 100, 100)),
    ]);

    let animated_rows: Vec<(usize, &str)> = banner_lines
        .iter()
        .enumerate()
        .map(|(i, line)| (i + 2, *line))
        .collect();

    let plasma = RowPlasma::new(
        animated_rows,
        presets::storm().palette(256),
        seed,
    );

    let effect = LayeredEffect::new()
        .add(static_text)
        .add(plasma);

    framebuffer::run_effect(effect, width, height, Duration::from_secs(10), 1.0).await;
}
