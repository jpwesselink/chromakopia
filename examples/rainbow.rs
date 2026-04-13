use chromakopia::animate::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = Rainbow::on("chromakopia: beautiful terminal animations").spawn();
    tokio::time::sleep(Duration::from_secs(3)).await;
    anim.fade_out(1.0);
    anim.wait().await;
}
