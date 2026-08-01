//! Marquee — one wrapper, five personalities.
//!
//! Every row below is the same `Marquee` effect with different knobs:
//! constant scroll (left/right), an eased cycle that moves in swipes,
//! a sine-wave swing, and a vertical credits crawl through a 3-row
//! window. Spaces are transparent, content wraps seamlessly.
//!
//! Run: `cargo run --release --example marquee`   (press q to quit)

use chromakopia::prelude::*;

const RIBBON: &str = "  CHROMAKOPIA ★ TERMINAL ANIMATIONS ★ EFFECTS ★ SCENES ★";
const STRIP: &str = "════════╡ chromakopia ╞════════════════════";
const SWING_LINE: &str = "◄ ─────────── swing ─────────── ►";
const CREDITS: &str = "\
      a chromakopia production      \n\
                                    \n\
       effects ... the library      \n\
        motion ....... Marquee      \n\
                                    \n\
      thanks for scrolling by       ";

#[tokio::main]
async fn main() {
    let label = Color::new(110, 110, 110);
    let fire = gradient(&["#331000", "#ff6a00", "#ffd000"]).palette(64);

    Scene::new()
        .add(text("Marquee — one wrapper, five personalities", Color::new(160, 160, 160)))
        .blank()
        .add(text("1. scroll left (default), constant speed", label))
        .add(Marquee::wrap(Rainbow::on(RIBBON)).speed(10.0))
        .blank()
        .add(text("2. scroll right, slower", label))
        .add(Marquee::wrap(Plasma::on(RIBBON).palette(fire))
            .direction(ScrollDirection::Right)
            .speed(6.0))
        .blank()
        .add(text("3. eased cycle — decelerating swipes (EaseOut every 2s)", label))
        .add(Marquee::wrap(text(STRIP, Color::new(230, 200, 120)))
            .cycle(2.0, Easing::EaseOut))
        .blank()
        .add(text("4. swing — sine oscillation, ±15 cells", label))
        .add(Marquee::wrap(Glow::on(SWING_LINE))
            .speed(12.0)
            .cycle(5.0, Easing::Linear)
            .swing())
        .blank()
        .add(text("5. vertical crawl — 6 lines through a 3-row window", label))
        .add(Marquee::wrap(Rainbow::on(CREDITS))
            .direction(ScrollDirection::Top)
            .speed(3.0)
            .size(36, 3))
        .quit_key('q')
        .run(30.0)
        .await;
}
