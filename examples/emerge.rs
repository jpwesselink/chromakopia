/// Text materializes from nothing, glows, then settles into a gradient.
/// All fades use easing curves for buttery-smooth transitions.
use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, TimeRange};

const BANNER: &str = r#"
   ___ _  _ ___  ___  __  __   _   _  ___  ___ ___   _
  / __| || | _ \/ _ \|  \/  | /_\ | |/ / _ \| _ \_ _| /_\
 | (__| __ |   / (_) | |\/| |/ _ \| ' < (_) |  _/| || / _ \
  \___|_||_|_|_\\___/|_|  |_/_/ \_\_|\_\___/|_| |___|/_/ \_\
"#;

#[tokio::main]
async fn main() {
    let text = BANNER.trim_start_matches('\n');

    animate::Sequence::new(text)
        // 8 seconds of mist glow
        .effect(
            TimeRange::new(0.0, 8.0),
            30,
            animate::glow_effect(presets::mist()),
        )
        // Slow, eased emergence from darkness (0-2.5s)
        .fade(
            TimeRange::new(0.0, 2.5),
            FadeKind::FadeFrom(FadeTarget::Background),
            Easing::CubicBezier(0.0, 0.0, 0.2, 1.0), // aggressive ease-out
        )
        // Settle into dark_n_stormy gradient (5.5-8s)
        .fade(
            TimeRange::new(5.5, 8.0),
            FadeKind::FadeTo(FadeTarget::Gradient(presets::dark_n_stormy())),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}
