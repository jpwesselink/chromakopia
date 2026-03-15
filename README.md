# shimmer

Beautiful terminal string gradients and animations for Rust. A port of [gradient-string](https://github.com/bokub/gradient-string) and [chalk-animation](https://github.com/bokub/chalk-animation), built on `colored` + `tokio`.

## Gradients

```rust
use shimmer::{gradient, presets};

// Custom gradient
println!("{}", gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello!"));

// HSV interpolation
println!("{}", gradient(&["cyan", "pink"]).hsv().apply("Smooth"));

// Presets
println!("{}", presets::rainbow().apply("Rainbow text"));

// Multiline (column-aligned, great for ASCII art)
println!("{}", presets::atlas().multiline(ascii_art));
```

17 presets: `atlas`, `cristal`, `teen`, `mind`, `morning`, `vice`, `passion`, `fruit`, `instagram`, `retro`, `summer`, `rainbow`, `pastel`, `dark_n_stormy`, `mist`, `relic`, `flughafen`

## Animations

Standalone animations run on a background tokio task with start/stop control:

```rust
use shimmer::animate;

let anim = animate::rainbow("Loading...", 1.0);
// ... do async work ...
anim.replace("Almost done...");
anim.stop();
```

Effects: `rainbow`, `pulse`, `glitch`, `radar`, `neon`, `karaoke`

Gradient-parameterized: `glow(gradient, ...)`, `cycle(gradient, ...)`

Split-flap board: `flap(...)`, `flap_with(gradient, ...)`

## Sequences

Chain animations with fades and transitions:

```rust
use shimmer::{animate, presets};
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
- `.flap(duration)` — split-flap departure board
- `.flap_with(gradient, duration)` — split-flap with custom colors
- `.hold(color, duration)` — static colored text
- `.fade_in(duration)` / `.fade_out(duration)` — fade from/to black

### Fade modifiers (applied to last step)

- `.with_fade(fade_in, fade_out)` — fade to/from background (text disappears)
- `.fade_to_foreground(duration)` — settle into terminal's text color
- `.fade_to_color(color, duration)` — settle into a specific color
- `.fade_to_gradient(gradient, duration)` — settle into a static gradient

### Terminal detection

Auto-detects terminal background (OSC 11) and foreground (OSC 10) colors for seamless fades. Override with `set_bg_color()` / `set_fg_color()`.

## Examples

```sh
cargo run --example demo          # static gradients and presets
cargo run --example ascii_art     # animated ASCII art banners
cargo run --example loading       # simulated CLI loading flow
cargo run --example sequence      # chained glow + rainbow with fades
cargo run --example settle        # glow settling into a gradient
cargo run --example flap_fade     # split-flap with fade
cargo run --example fade_in       # fade-in effect

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
