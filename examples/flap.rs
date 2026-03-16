use chromakopia::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::flap("AMSTERDAM  15:42  GATE B7  ON TIME", 1.0);
    tokio::time::sleep(Duration::from_secs(6)).await;
    anim.stop();
}
