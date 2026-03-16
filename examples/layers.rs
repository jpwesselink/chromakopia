/// Demonstrates the power-user layer API: overlapping effects and fades
/// on an explicit timeline with easing curves.
use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, TimeRange};
use std::time::Duration;

const BANNER: &str = r#"       __    __
.-----|  |--|__.--------.--------.-----.----.
|__ --|     |  |        |        |  -__|   _|
|_____|__|__|__|__|__|__|__|__|__|_____|__|"#;

#[tokio::main]
async fn main() {
    // Place a glow effect from 0-5s, with an eased fade-in from 0-1s
    // and an eased fade-to-gradient from 3-5s — all as explicit layers.
    animate::Sequence::new(BANNER)
        .effect(
            TimeRange::from_duration(Duration::ZERO, Duration::from_secs(5)),
            30,
            animate::glow_effect(presets::mist()),
        )
        .fade(
            TimeRange::from_duration(Duration::ZERO, Duration::from_secs(1)),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::from_duration(Duration::from_secs(3), Duration::from_secs(5)),
            FadeKind::FadeTo(FadeTarget::Gradient(presets::dark_n_stormy())),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}
