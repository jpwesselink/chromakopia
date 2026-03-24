use chromakopia::animate;
use std::time::Duration;

const BANNER: &str = r#"
    __  _______  ____  _   __
   /  |/  / __ \/ __ \/ | / /
  / /|_/ / / / / / / /  |/ /
 / /  / / /_/ / /_/ / /|  /
/_/  /_/\____/\____/_/ |_/
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');
    let anim = animate::petscii("🌑🌒🌓🌔🌕🌖🌗🌘", text, 0.5);
    tokio::time::sleep(Duration::from_secs(10)).await;
    anim.stop();
}
