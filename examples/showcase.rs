/// The badass demo. Run this one for the README recording.
///
/// Four acts in ~18 seconds:
/// 1. Flap reveal of the banner in warm amber (airport board style)
/// 2. Cross-fade into plasma storm
/// 3. Cross-fade into a slow rainbow
/// 4. Settle into a static gradient with the tagline visible
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
        // ── Act 1: Flap reveal (0–5s) ──
        // Letters click into place like an airport departure board
        .effect(
            TimeRange::new(0.0, 5.0), 60,
            animate::flap_effect(flap_settled, flap_flip),
        )
        .fade(
            TimeRange::new(0.0, 1.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(4.0, 5.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 2: Plasma storm (5–10s) ──
        // Flowing demoscene colors wash over the text
        .effect(
            TimeRange::new(5.0, 10.0), 30,
            animate::plasma_gradient_effect(presets::storm()),
        )
        .fade(
            TimeRange::new(5.0, 6.0),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
        )
        .fade(
            TimeRange::new(9.0, 10.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 3: Rainbow burst (10–15s) ──
        // Fast rainbow hue rotation
        .effect(
            TimeRange::new(10.0, 15.0), 15,
            animate::rainbow_effect(),
        )
        .fade(
            TimeRange::new(10.0, 11.0),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(13.5, 15.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 4: Glow settle (15–20s) ──
        // Gentle glow that fades into a static gradient
        .effect(
            TimeRange::new(15.0, 20.0), 30,
            animate::glow_effect(presets::mist()),
        )
        .fade(
            TimeRange::new(15.0, 16.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(17.5, 20.0),
            FadeKind::FadeTo(FadeTarget::Gradient(presets::instagram())),
            Easing::EaseInOut,
        )

        .run(1.0)
        .await;

    // Hold the settled gradient for a beat
    tokio::time::sleep(Duration::from_secs(2)).await;
}
