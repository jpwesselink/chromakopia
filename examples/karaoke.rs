use chromakopia::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::karaoke("Is this the real life? Is this just fantasy?", 1.5);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
