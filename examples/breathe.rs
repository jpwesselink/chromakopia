/// Breathing pulse: text fades in and out rhythmically with eased
/// transitions, then settles into the terminal's foreground color.
use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, TimeRange};

const BANNER: &str = r#"
 ╔══════════════════════════════════════╗
 ║   breathe in ... breathe out ...    ║
 ╚══════════════════════════════════════╝
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_start_matches('\n');
    let pulse = 2.5; // seconds per half-cycle
    let cycles = 3;
    let total = pulse * (cycles as f64 * 2.0);

    let mut seq = animate::Sequence::new(text)
        .effect(
            TimeRange::new(0.0, total),
            30,
            animate::glow_effect(presets::relic()),
        );

    // 3 breathing cycles: fade-in then fade-out
    for i in 0..cycles {
        let base = i as f64 * pulse * 2.0;

        seq = seq.fade(
            TimeRange::new(base, base + pulse),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::EaseInOut,
        );

        if i < cycles - 1 {
            // Normal cycles: fade back to background
            seq = seq.fade(
                TimeRange::new(base + pulse, base + pulse * 2.0),
                FadeKind::FadeTo(FadeTarget::Background),
                Easing::EaseInOut,
            );
        } else {
            // Last cycle: settle into foreground
            seq = seq.fade(
                TimeRange::new(base + pulse, base + pulse * 2.0),
                FadeKind::FadeTo(FadeTarget::Foreground),
                Easing::EaseOut,
            );
        }
    }

    seq.run(1.0).await;
}
