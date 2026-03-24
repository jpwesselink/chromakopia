use chromakopia::{animate, presets};
use std::time::Duration;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');
    let anim = animate::scroll_with(
        presets::storm(),
        text,
        Duration::from_secs(2),
        1.0,
    );
    // Hold after the bounce settles
    tokio::time::sleep(Duration::from_secs(4)).await;
    anim.stop();
}
