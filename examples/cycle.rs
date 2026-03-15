use shimmer::{animate, presets};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::cycle(presets::instagram(), "shimmer: beautiful terminal animations", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
