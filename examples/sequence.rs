use chromakopia::{animate, presets};
use std::time::Duration;

const BANNER: &str = r#"       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|"#;

#[tokio::main]
async fn main() {
    animate::Sequence::new(BANNER)
        .glow(presets::dark_n_stormy(), Duration::from_secs(5))
        .with_fade(Duration::from_secs(1), Duration::from_secs(1))
        .glow(presets::mist(), Duration::from_secs(5))
        .with_fade(Duration::from_secs(1), Duration::from_secs(1))
        .rainbow(Duration::from_secs(3))
        .with_fade(Duration::ZERO, Duration::from_secs(1))
        .run(1.0)
        .await;
}
