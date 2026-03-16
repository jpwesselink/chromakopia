use chromakopia::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::neon("OPEN 24 HOURS", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
