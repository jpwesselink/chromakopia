use chromakopia::{animate, presets};
use std::time::Duration;

#[tokio::main]
async fn main() {
    eprintln!("--- dark_n_stormy ---");
    let anim = animate::glow(presets::dark_n_stormy(), "chromakopia: beautiful terminal animations", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;

    eprintln!("--- mist ---");
    let anim = animate::glow(presets::mist(), "chromakopia: beautiful terminal animations", 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
}
