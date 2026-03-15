use shimmer::animate;
use std::time::Duration;

#[tokio::main]
async fn main() {
    animate::Sequence::new("AMSTERDAM  15:42  GATE B7  ON TIME")
        .flap(Duration::from_secs(4))
        .with_fade(Duration::from_secs(1), Duration::ZERO)
        .hold(shimmer::Color::new(0xff, 0xcc, 0x00), Duration::from_secs(2))
        .with_fade(Duration::ZERO, Duration::from_secs(1))
        .run(1.0)
        .await;
}
