use chromakopia::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let anim = animate::radar("Scanning dependencies for vulnerabilities...", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
