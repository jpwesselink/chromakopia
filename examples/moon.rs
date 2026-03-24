use chromakopia::animate;
use std::time::Duration;

const MOON: &str = "\
🌕🌑 🌑🌕 🌑🌕🌕🌕🌑 🌑🌕🌕🌕🌑 🌕🌑  🌑🌕
🌕🌕🌑🌕🌕 🌕🌑   🌑🌕 🌕🌑   🌑🌕 🌕🌕🌑🌕🌕
🌕🌑🌕🌑🌕 🌕🌑   🌑🌕 🌕🌑   🌑🌕 🌕🌑🌕🌑🌕
🌕🌑 🌑🌕 🌑🌕🌕🌕🌑 🌑🌕🌕🌕🌑 🌕🌑  🌑🌕";

#[tokio::main]
async fn main() {
    let anim = animate::petscii("🌑🌒🌓🌔🌕🌖🌗🌘", MOON, 0.3);
    tokio::time::sleep(Duration::from_secs(10)).await;
    anim.stop();
}
