use chromakopia::{animate, presets};
use std::time::Duration;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

const TAGLINE: &str =
    "  MIT License (c) 2026 JP Wesselink https://github.com/jpwesselink/chromakopia";

#[tokio::main]
async fn main() {
    // Banner slides in first
    let banner = BANNER.trim_matches('\n');
    let anim = animate::scroll_with(
        presets::storm(),
        banner,
        Duration::from_secs(2),
        1.0,
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    anim.stop();

    // Then tagline slides in below
    eprintln!();
    let anim = animate::scroll_with(
        presets::mist(),
        TAGLINE,
        Duration::from_millis(1500),
        1.0,
    );
    tokio::time::sleep(Duration::from_secs(3)).await;
    anim.stop();
}
