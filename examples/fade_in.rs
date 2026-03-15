use shimmer::{animate, presets};
use std::time::Duration;

const BANNER: &str = r#"       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|"#;

#[tokio::main]
async fn main() {
    // Fade in to glow, then fade out
    animate::Sequence::new(BANNER)
        .glow(presets::dark_n_stormy(), Duration::from_secs(5))
        .with_fade(Duration::from_secs(2), Duration::from_secs(2))
        .run(1.0)
        .await;

    eprintln!();

    // Fade in to static mist, hold, fade out
    animate::Sequence::new(BANNER)
        .hold(shimmer::Color::new(0x8f, 0xcd, 0xdb), Duration::from_secs(4))
        .with_fade(Duration::from_secs(2), Duration::from_secs(2))
        .run(1.0)
        .await;

    eprintln!();

    // Fade in to rainbow, hold, fade out
    animate::Sequence::new("shimmer: beautiful terminal animations")
        .rainbow(Duration::from_secs(5))
        .with_fade(Duration::from_secs(1), Duration::from_secs(1))
        .run(1.0)
        .await;
}
