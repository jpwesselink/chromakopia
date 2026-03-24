use chromakopia::animate::{Scene, Line, Rainbow, Plasma, Sparkle, Glow};
use chromakopia::{presets, Color};
use std::time::Duration;

const BANNER: &str = r#"   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|"#;

#[tokio::main]
async fn main() {
    let gray = Color::new(100, 100, 100);
    let storm_pal = presets::storm().palette(256);
    let star_pal = presets::starfield().palette(64);
    let mist_pal = presets::mist().palette(256);

    Scene::new()
        .line(Line::new()
            .text("(c) 2026 ", gray)
            .animated("JP Wesselink", Rainbow::new("JP Wesselink"))
        )
        .line(Line::blank())
        .text_block(BANNER, |l| {
            Line::full(l, Plasma::new(l, storm_pal.clone(), 42.0))
        })
        .line(Line::blank())
        .line(Line::new()
            .animated("github.com/jpwesselink/chromakopia", Glow::new("github.com/jpwesselink/chromakopia", mist_pal.clone()))
            .text("  ", gray)
            .animated("My God, it's full of stars.", Sparkle::new("My God, it's full of stars.", star_pal.clone()))
        )
        .run(Duration::from_secs(10))
        .await;
}
