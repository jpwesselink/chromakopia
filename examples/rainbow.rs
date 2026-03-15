use shimmer::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::rainbow("shimmer: beautiful terminal animations", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
