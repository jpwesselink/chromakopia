# chromakopia

Beautiful terminal string gradients and animations for Rust. A port of [gradient-string](https://github.com/bokub/gradient-string) and [chalk-animation](https://github.com/bokub/chalk-animation), built on `colored` + `tokio`.

## Gradients

```rust
use chromakopia::{gradient, presets};

// Custom gradient
println!("{}", gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello!"));

// HSV interpolation
println!("{}", gradient(&["cyan", "pink"]).hsv().apply("Smooth"));

// Presets
println!("{}", presets::rainbow().apply("Rainbow text"));

// Multiline (column-aligned, great for ASCII art)
println!("{}", presets::atlas().multiline(ascii_art));
```

18 presets: `atlas`, `cristal`, `teen`, `mind`, `morning`, `vice`, `passion`, `fruit`, `instagram`, `retro`, `summer`, `rainbow`, `pastel`, `dark_n_stormy`, `mist`, `relic`, `storm`, `flughafen`

## Animations

Standalone animations run on a background tokio task with start/stop control:

```rust
use chromakopia::animate;

let anim = animate::rainbow("Loading...", 1.0);
// ... do async work ...
anim.replace("Almost done...");
anim.stop();
```

Effects: `rainbow`, `pulse`, `glitch`, `radar`, `neon`, `karaoke`

Gradient-parameterized: `glow(gradient, ...)`, `cycle(gradient, ...)`, `plasma(...)`, `plasma_with(gradient, ...)`

Split-flap board: `flap(...)`, `flap_with(gradient, ...)`

## Sequences

Chain animations with fades and transitions:

```rust
use chromakopia::{animate, presets};
use std::time::Duration;

animate::Sequence::new("Hello, world!")
    .glow(presets::mist(), Duration::from_secs(5))
    .with_fade(Duration::from_secs(1), Duration::ZERO)
    .fade_to_gradient(presets::dark_n_stormy(), Duration::from_secs(2))
    .run(1.0)
    .await;
```

### Sequence steps

- `.glow(gradient, duration)` — sweeping glow
- `.rainbow(duration)` — HSV hue shift
- `.cycle(gradient, duration)` — scrolling gradient
- `.plasma(duration)` — demoscene-style flowing color field
- `.plasma_with(gradient, duration)` — plasma with custom gradient
- `.flap(duration)` — split-flap departure board
- `.flap_with(gradient, duration)` — split-flap with custom colors
- `.hold(color, duration)` — static colored text
- `.fade_in(duration)` / `.fade_out(duration)` — fade from/to black

### Fade modifiers (applied to last step)

- `.with_fade(fade_in, fade_out)` — fade to/from background (text disappears)
- `.fade_to_foreground(duration)` — settle into terminal's text color
- `.fade_to_color(color, duration)` — settle into a specific color
- `.fade_to_gradient(gradient, duration)` — settle into a static gradient
- `.eased(Easing)` — apply an easing curve to the last fade

### Easing curves

All fade transitions support easing via `.eased()`:

```rust
use chromakopia::animate::Easing;

animate::Sequence::new("Hello!")
    .glow(presets::mist(), Duration::from_secs(5))
    .with_fade(Duration::from_secs(1), Duration::ZERO)
    .eased(Easing::EaseOut)
    .fade_to_gradient(presets::dark_n_stormy(), Duration::from_secs(2))
    .eased(Easing::EaseInOut)
    .run(1.0)
    .await;
```

Built-in curves: `Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, `CubicBezier(x1, y1, x2, y2)`

### Layer API (power-user)

Place effects and fades at explicit time ranges for full compositional control:

```rust
use chromakopia::animate::{Sequence, TimeRange, FadeKind, FadeTarget, Easing};

Sequence::new("Hello!")
    .effect(TimeRange::new(0.0, 5.0), 30, animate::glow_effect(presets::mist()))
    .fade(
        TimeRange::new(0.0, 1.0),
        FadeKind::FadeFrom(FadeTarget::Background),
        Easing::EaseOut,
    )
    .fade(
        TimeRange::new(3.0, 5.0),
        FadeKind::FadeTo(FadeTarget::Gradient(presets::dark_n_stormy())),
        Easing::EaseInOut,
    )
    .run(1.0)
    .await;
```

Effect factories: `rainbow_effect()`, `glow_effect(gradient)`, `cycle_effect(gradient)`, `flap_effect(settled, flipping)`, `plasma_effect()`, `plasma_gradient_effect(gradient)`

### Terminal detection

Auto-detects terminal colors using a 5-tier fallback chain:

1. OSC 10/11 query via `/dev/tty` (exact RGB)
2. `COLORFGBG` environment variable
3. `TERM_PROGRAM` heuristics (known terminal defaults)
4. macOS system theme (`defaults read`)
5. Hardcoded defaults

```rust
use chromakopia::{bg_color, is_dark_theme, Color};

let bg = bg_color();
println!("Background luma: {:.2}", bg.luma());

if is_dark_theme() {
    // use light gradients
} else {
    // use dark gradients
}
```

Override with `set_bg_color()` / `set_fg_color()`.

## Examples

```sh
cargo run --example demo             # static gradients and presets
cargo run --example ascii_art        # animated ASCII art banners
cargo run --example loading          # simulated CLI loading flow
cargo run --example theme_adaptive   # adapts to light/dark terminal theme
cargo run --example sequence         # chained glow + rainbow with fades
cargo run --example settle           # glow settling into gradient with easing
cargo run --example layers           # power-user layer API with explicit time ranges
cargo run --example cinematic        # multi-act sequence with custom easing
cargo run --example plasma           # demoscene-style plasma effect
cargo run --example breathe          # rhythmic breathing pulse
cargo run --example emerge           # text materializing from darkness
cargo run --example flap_fade        # split-flap with fade
cargo run --example fade_in          # fade-in effect

# Individual animations
cargo run --example rainbow
cargo run --example pulse
cargo run --example glitch
cargo run --example radar
cargo run --example neon
cargo run --example karaoke
cargo run --example glow
cargo run --example cycle
cargo run --example flap
```
