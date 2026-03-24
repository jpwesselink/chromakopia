/// The badass demo. Run this one for the README recording.
///
/// Five acts in ~23 seconds:
/// 1. Slide-in with bounce (text enters from the left)
/// 2. Flap reveal in warm amber (airport board style)
/// 3. Plasma storm (demoscene flowing colors)
/// 4. Rainbow burst
/// 5. Glow settle into a static gradient
use chromakopia::{animate, presets, Color};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, TimeRange};
use std::time::Duration;

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
     (c) 2026 JP Wesselink — github.com/jpwesselink/chromakopia — MIT License
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_matches('\n');

    // Clear screen and move cursor to top-left
    eprint!("\x1B[2J\x1B[H");

    let flap_settled = Color::new(0xff, 0xcc, 0x00);
    let flap_flip = Color::new(0x99, 0x7a, 0x00);

    animate::Sequence::new(text)
        // ── Act 1: Slide-in with bounce (0–3s) ──
        // Text enters from the left with a bounce at the end
        .effect(
            TimeRange::new(0.0, 3.0), 30,
            animate::scroll_gradient_effect(animate::ScrollDirection::Left, presets::storm(), 60),
        )
        .fade(
            TimeRange::new(2.5, 3.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 2: Flap reveal (3–8s) ──
        // Letters click into place like an airport departure board
        .effect(
            TimeRange::new(3.0, 8.0), 60,
            animate::flap_effect(flap_settled, flap_flip),
        )
        .fade(
            TimeRange::new(3.0, 4.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(7.0, 8.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 3: Plasma storm (8–13s) ──
        // Flowing demoscene colors wash over the text
        .effect(
            TimeRange::new(8.0, 13.0), 30,
            animate::plasma_gradient_effect(presets::storm()),
        )
        .fade(
            TimeRange::new(8.0, 9.0),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
        )
        .fade(
            TimeRange::new(12.0, 13.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 4: Rainbow burst (13–18s) ──
        // Fast rainbow hue rotation
        .effect(
            TimeRange::new(13.0, 18.0), 15,
            animate::rainbow_effect(),
        )
        .fade(
            TimeRange::new(13.0, 14.0),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(16.5, 18.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 5: Glow settle (18–23s) ──
        // Gentle glow that fades into a static gradient
        .effect(
            TimeRange::new(18.0, 23.0), 30,
            animate::glow_effect(presets::mist()),
        )
        .fade(
            TimeRange::new(18.0, 19.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(20.5, 23.0),
            FadeKind::FadeTo(FadeTarget::Gradient(presets::instagram())),
            Easing::EaseInOut,
        )

        .run(1.0)
        .await;

    // Hold the settled gradient for a beat
    tokio::time::sleep(Duration::from_secs(2)).await;
}
