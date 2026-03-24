use chromakopia::animate::fb_effects::{Scene, SceneLine};
use chromakopia::presets;
use chromakopia::Color;
use std::time::Duration;

const BANNER: &str = "\
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \\/ __ \\/  |/  /   |  / //_/ __ \\/ __ \\/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\\____/_/ /_/_/ |_|\\____/_/  /_/_/  |_/_/ |_\\____/_/   /___/_/  |_|";

#[tokio::main]
async fn main() {
    Scene::new()
        .seed(rand::random::<f64>() * 1000.0)
        .line(SceneLine::Static("(c) 2026 JP Wesselink", Color::new(100, 100, 100)))
        .line(SceneLine::Blank)
        .text_block(BANNER, |line| SceneLine::Plasma(line, presets::storm()))
        .line(SceneLine::Blank)
        .line(SceneLine::Static("github.com/jpwesselink/chromakopia", Color::new(100, 100, 100)))
        .run(Duration::from_secs(10))
        .await;
}
