use chromakopia::animate::*;
use chromakopia::{center, presets};
use std::time::Duration;

const BANNER: &str = r#"   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|"#;

const CREDIT: &str = "(c) 2026 JP Wesselink";
const FOOTER: &str = "github.com/jpwesselink/chromakopia  —  MIT License";

#[tokio::main]
async fn main() {
    let bg = chromakopia::bg_color();
    let storm = presets::storm().palette(256);
    let mist = presets::mist().palette(256);
    let fps = 60;

    // Center everything as a block
    let full = format!("{}\n\n{}\n\n{}", CREDIT, BANNER, FOOTER);
    let centered = center(&full);
    let lines: Vec<&str> = centered.lines().collect();

    // Lines layout: 0=credit, 1=blank, 2-6=banner, 7=blank, 8=footer
    let credit = lines[0];
    let banner_lines = &lines[2..7];
    let footer = lines[8];

    let mut scene = Scene::new();

    // Credit — fades in over rainbow
    scene = scene.line(Line::full(credit,
        Fade::in_from(Rainbow::new(credit), bg, Easing::EaseOut, fps)
    ));
    scene = scene.line(Line::blank());

    // Banner — scroll (position) + plasma (color) composited, with fade, aggressive elastic
    for l in banner_lines {
        scene = scene.line(Line::full(l, Chain::new()
            .then(fps * 3, Fade::in_from(
                Composite::new(
                    Scroll::new(l, storm.clone(), ScrollDirection::Left, Easing::Elastic(0.15), fps * 3, 1),
                    Plasma::new(l, storm.clone(), 42.0),
                ),
                bg, Easing::EaseOut, fps,
            ))
            .then(fps * 100, Plasma::new(l, storm.clone(), 42.0))
        ));
    }

    scene = scene.line(Line::blank());

    // Footer — fades in over a glow
    scene = scene.line(Line::full(footer, Chain::new()
        .then(fps * 2, Fade::in_from(
            Glow::new(footer, mist.clone()),
            bg, Easing::EaseOut, fps * 2,
        ))
        .then(fps * 100, Glow::new(footer, mist.clone()))
    ));

    scene.run(Duration::from_secs(15)).await;
}
