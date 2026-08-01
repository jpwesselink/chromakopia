use chromakopia::animate::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = Rainbow::on("chromakopia: beautiful terminal animations")
        .quit_key('q')
        .spawn();
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(3)) => anim.fade_out(1.0),
        _ = anim.wait() => {}
    }
    anim.wait().await;
}
