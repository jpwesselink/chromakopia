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
    let anim = animate::plasma_with(presets::storm(), text, 1.0);
    tokio::time::sleep(Duration::from_secs(10)).await;
    anim.stop();
}
