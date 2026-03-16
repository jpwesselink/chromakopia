use chromakopia::{animate, presets};
use chromakopia::animate::Easing;
use std::time::Duration;

const BANNER: &str = r#"       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|"#;

#[tokio::main]
async fn main() {
    animate::Sequence::new(BANNER)
        .glow(presets::mist(), Duration::from_secs(5))
        .with_fade(Duration::from_secs(1), Duration::ZERO)
        .eased(Easing::EaseOut)
        .fade_to_gradient(presets::dark_n_stormy(), Duration::from_secs(2))
        .eased(Easing::EaseInOut)
        .run(1.0)
        .await;
}
