//! Marquee — one wrapper, six personalities.
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
/// Figlet `basic`-font wordmark for the ripple row (`figlet -f basic`):
/// every letter is a self-contained block with a gap column, so the
/// wave's per-column shear bends the word without tearing glyphs apart.
/// Two blank rows above and below give the ±2-cell wave its headroom.
/// (`lines()` drops a lone trailing empty line, hence the explicit extra
/// blank row before the closing quote.)
const BANNER_WAVE: &str = r#"

 .o88b. db   db d8888b.  .d88b.  .88b  d88.  .d8b.  db   dD  .d88b.  d8888b. d888888b  .d8b.
d8P  Y8 88   88 88  `8D .8P  Y8. 88'YbdP`88 d8' `8b 88 ,8P' .8P  Y8. 88  `8D   `88'   d8' `8b
8P      88ooo88 88oobY' 88    88 88  88  88 88ooo88 88,8P   88    88 88oodD'    88    88ooo88
8b      88~~~88 88`8b   88    88 88  88  88 88~~~88 88`8b   88    88 88~~~      88    88~~~88
Y8b  d8 88   88 88 `88. `8b  d8' 88  88  88 88   88 88 `88. `8b  d8' 88        .88.   88   88
 `Y88P' YP   YP 88   YD  `Y88P'  YP  YP  YP YP   YP YP   YD  `Y88P'  88      Y888888P YP   YP


"#;
const SWING_LINE: &str = "◄ ─────────── swing ─────────── ►";
// Three blank rows at the end: the vertical marquee wraps seamlessly, so
// without a gap the last line and the first line would sit adjacent in
// the window and the loop would read as a jumble.
const CREDITS: &str = "\
      a chromakopia production      \n\
                                    \n\
       effects ... the library      \n\
        motion ....... Marquee      \n\
                                    \n\
      thanks for scrolling by       \n\n\n";

#[tokio::main]
async fn main() {
    let label = Color::new(110, 110, 110);
    let fire = gradient(&["#331000", "#ff6a00", "#ffd000"]).palette(64);

    // Pad every banner line to 135 cols: the basic-font wordmark is ~95
    // cols, so this leaves a ~40-col breather after "CHROMAKOPIA" before
    // the loop repeats (Marquee wraps at the content's max line width).
    let banner_wave: String = BANNER_WAVE
        .lines()
        .map(|l| format!("{l:<135}"))
        .collect::<Vec<_>>()
        .join("\n");

    Scene::new()
        .add(text("Marquee — one wrapper, six personalities", Color::new(160, 160, 160)))
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
        .add(text("5. vertical crawl — credits loop through a 3-row window", label))
        .add(Marquee::wrap(Rainbow::on(CREDITS))
            .direction(ScrollDirection::Top)
            .speed(3.0)
            .size(36, 3))
        .blank()
        .add(text("6. scroll + VSineWave — the ripple marquee", label))
        // Wave speed detuned from the phase-lock (lock would be
        // frequency * scroll_cells_per_sec / FPS): crests scream along at
        // 44 cells/s over text scrolling at 8 — redline. Beyond ~45 the
        // 30fps renderer skips >1.5 cells of phase per frame and the
        // ripple strobes instead of rolling.
        // Wavelength = 2π/0.08 ≈ 79 cols — gentle shear across the glyphs.
        .add(VSineWave::wrap(Marquee::wrap(Rainbow::on(&banner_wave)).speed(8.0))
            .amplitude(2.0)
            .frequency(0.08)
            .speed(0.08 * 44.0 / 30.0))
        .quit_key('q')
        .run(30.0)
        .await;
}
