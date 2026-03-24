use chromakopia::animate::*;
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
    let bg = chromakopia::bg_color();
    let storm = presets::storm().palette(256);
    let mist = presets::mist().palette(256);
    let fps = 60;

    Scene::new()
        // Credit — fades in over rainbow
        .line(Line::new()
            .text("(c) 2026 ", gray)
            .animated("JP Wesselink",
                Fade::in_from(Rainbow::new("JP Wesselink"), bg, Easing::EaseOut, fps)
            )
        )
        .line(Line::blank())
        // Banner — scrolls in, then crossfades into plasma
        .text_block(BANNER, |l| {
            Line::full(l, Chain::new()
                .then(fps * 2, Fade::in_from(
                    Scroll::new(l, storm.clone(), ScrollDirection::Left, Easing::Elastic(0.3), fps * 2, 1),
                    bg, Easing::EaseOut, fps,
                ))
                .then(fps * 100, Plasma::new(l, storm.clone(), 42.0))
            )
        })
        .line(Line::blank())
        // Footer — fades in over a glow
        .line(Line::full(
            "github.com/jpwesselink/chromakopia  —  MIT License",
            Chain::new()
                .then(fps * 2, Fade::in_from(
                    Glow::new("github.com/jpwesselink/chromakopia  —  MIT License", mist.clone()),
                    bg, Easing::EaseOut, fps * 2,
                ))
                .then(fps * 100, Glow::new(
                    "github.com/jpwesselink/chromakopia  —  MIT License", mist.clone(),
                ))
        ))
        .run(Duration::from_secs(15))
        .await;
}
