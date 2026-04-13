use chromakopia::animate::*;
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
    let anim = Plasma::on(text).spawn();
    tokio::time::sleep(Duration::from_secs(8)).await;
    anim.fade_out(2.0);
    anim.wait().await;
}
