use chromakopia::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::glitch("SYSTEM OVERRIDE IN PROGRESS", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
