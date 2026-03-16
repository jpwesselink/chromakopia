/// Multi-act cinematic sequence using explicit layer placement.
///
/// Act 1: Split-flap reveal (airport board style)
/// Act 2: Mist glow with eased cross-fade
/// Act 3: Rainbow burst, settling into a warm gradient
use chromakopia::{animate, presets, Color};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, TimeRange};

const BANNER: &str = r#"
 ┌─────────────────────────────────────────┐
 │  FLIGHT 747  ·  DESTINATION: TOMORROW   │
 │  GATE A12    ·  STATUS: ON TIME         │
 └─────────────────────────────────────────┘
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_start_matches('\n');

    let flap_settled = Color::new(0xff, 0xcc, 0x00);
    let flap_flip = Color::new(0x99, 0x7a, 0x00);

    animate::Sequence::new(text)
        // ── Act 1: Flap reveal (0-5s) ──
        .effect(
            TimeRange::new(0.0, 5.0),
            60,
            animate::flap_effect(flap_settled, flap_flip),
        )
        .fade(
            TimeRange::new(0.0, 1.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        // Fade out flap before glow starts
        .fade(
            TimeRange::new(4.0, 5.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 2: Mist glow (5-11s) ──
        .effect(
            TimeRange::new(5.0, 11.0),
            30,
            animate::glow_effect(presets::mist()),
        )
        .fade(
            TimeRange::new(5.0, 6.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseOut,
        )
        .fade(
            TimeRange::new(10.0, 11.0),
            FadeKind::FadeTo(FadeTarget::Background),
            Easing::EaseIn,
        )

        // ── Act 3: Rainbow burst → settle (11-17s) ──
        .effect(
            TimeRange::new(11.0, 17.0),
            15,
            animate::rainbow_effect(),
        )
        .fade(
            TimeRange::new(11.0, 12.0),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
        )
        // Settle into warm gradient
        .fade(
            TimeRange::new(14.5, 17.0),
            FadeKind::FadeTo(FadeTarget::Gradient(presets::dark_n_stormy())),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}
