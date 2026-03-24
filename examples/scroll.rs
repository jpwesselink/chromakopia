use chromakopia::{animate, center, presets};
use chromakopia::animate::{Easing, ScrollDirection, TimeRange};

const LINE1: &str = "MIT License (c) 2026 JP Wesselink";

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

const LINE3: &str = "github.com/jpwesselink/chromakopia  crates.io/crates/chromakopia";

#[tokio::main]
async fn main() {
    let banner = BANNER.trim_matches('\n');
    let full_text = center(&format!("{}\n\n{}\n\n{}", LINE1, banner, LINE3));

    animate::Sequence::new(&full_text)
        .effect(
            TimeRange::new(0.0, 6.0), 30,
            animate::scroll_staggered_effect(
                ScrollDirection::Left,
                Easing::Elastic(0.4),
                presets::storm(),
                60,  // 2s per line
                1,   // 1 frame stagger — slant effect
            ),
        )
        .run(1.0)
        .await;
}
