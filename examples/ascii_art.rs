/// Animated ASCII art banners.
use shimmer::{animate, presets};
use std::time::Duration;

const BANNER: &str = r#"
       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|"#;

const SMALL: &str = r#"
 ╔═══════════════════════════════════╗
 ║  shimmer — terminal gradients 🎨  ║
 ╚═══════════════════════════════════╝"#;

#[tokio::main]
async fn main() {
    // Static gradients
    eprintln!("=== dark_n_stormy ===\n");
    eprintln!(
        "{}",
        presets::dark_n_stormy().multiline(BANNER.trim_start_matches('\n'))
    );
    eprintln!();

    eprintln!("=== mist ===\n");
    eprintln!(
        "{}",
        presets::mist().multiline(BANNER.trim_start_matches('\n'))
    );
    eprintln!();

    eprintln!("=== relic ===\n");
    eprintln!(
        "{}",
        presets::relic().multiline(BANNER.trim_start_matches('\n'))
    );
    eprintln!();

    // Animated glow
    eprintln!("=== glow (dark_n_stormy) ===\n");
    let anim = animate::glow(presets::dark_n_stormy(), BANNER.trim_start_matches('\n'), 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!();

    eprintln!("=== glow (mist) ===\n");
    let anim = animate::glow(presets::mist(), BANNER.trim_start_matches('\n'), 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!();

    eprintln!("=== glow (relic) ===\n");
    let anim = animate::glow(presets::relic(), BANNER.trim_start_matches('\n'), 1.0);
    tokio::time::sleep(Duration::from_secs(5)).await;
    anim.stop();
    tokio::time::sleep(Duration::from_millis(100)).await;
    eprintln!();

    // Neon box
    eprintln!("=== neon ===\n");
    let anim = animate::neon(SMALL.trim_start_matches('\n'), 1.0);
    tokio::time::sleep(Duration::from_secs(4)).await;
    anim.stop();
}
