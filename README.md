# chromakopia

Framebuffer-based terminal animation engine for Rust. 30 FPS lockstep renderer, composable effects, same code targets the terminal and WASM.

```rust
use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    Rainbow::on("Hello, world!").run(3.0).await;
}
```

[Live demos →](https://jpwesselink.github.io/chromakopia/)

## Install

```toml
[dependencies]
chromakopia = "0.2"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

## Why a framebuffer

Every frame is a 2D grid of `Cell { ch, fg, bg }` rendered atomically via absolute cursor positioning. That gives you:

- **2D effects** — half-blocks (`▀ ▄ █` = 2 pixels per cell), per-cell colour fields, overlays composited over backgrounds.
- **Steady FPS** — lockstep producer/consumer at 30 FPS. Animation timing is computed in *frames* so the same code produces the same output on every machine.
- **In-place updates** — animations don't scroll the terminal; they redraw the same region.
- **WASM-portable** — the buffer is just a `Vec<Cell>`; effects render identically to a browser canvas via [chromakopia-wasm](https://jpwesselink.github.io/chromakopia/).

## Effects

`.on("text")` gives an effect text and a size:

```rust
Rainbow::on("text").spawn();        // HSV hue rotation
Plasma::on("text").spawn();         // demoscene sine waves
Neon::on("text").spawn();           // flickering bright/dim
Pulse::on("text").spawn();          // expanding red highlight
Radar::on("text").spawn();          // sweeping spotlight
Glow::on("text").spawn();           // gradient spotlight
Karaoke::on("text").spawn();        // progressive reveal
Glitch::on("text").spawn();         // random char corruption
Flap::on("text").spawn();           // split-flap board
```

New in v0.2:

```rust
Dycp::new("text").spawn();          // per-char vertical sine (DYCP)
Spread::new("text").spawn();        // letters fan out from a stacked start
Wind::wrap(effect).delay(2.0)       // cloth-dissolve: text tears off
    .duration(4.0)
    .angle_deg(315.0)
    .spawn();
Fld::new(effect)                    // demoscene flexible line distance
    .amplitude(2.0)
    .ramp(30);
```

Customise with builders — `.on()` works anywhere in the chain:

```rust
Plasma::on("text").palette(presets::storm().palette(256)).seed(42.0);
Plasma::new().palette(presets::storm().palette(256)).on("text");
```

## Scenes

Stack effects vertically with `Scene`. Overlay puts effects on top:

```rust
let white = Color::new(255, 255, 255);

Scene::new()
    .add(text("MIT License", white))
    .blank()
    .add(Rainbow::on("colored heading"))
    .add(Plasma::on("body text\nwith multiple lines"))
    .overlay(Dycp::new("watermark"), 1, 0)
    .run(5.0)
    .await;
```

## Composition

Effects compose. Blend, transition, chain, fade — then `.on("text")`:

```rust
// Blend two effects
Blend::new(Plasma::new(), Radar::new(), BlendMode::Screen)
    .on("blended text")
    .spawn();

// Chain effects sequentially
Chain::new()
    .then(90, Rainbow::new())
    .then(90, Neon::new())
    .on("chained text")
    .run(6.0).await;

// Fade in from black
Fade::in_from(Plasma::new(), Color::new(0, 0, 0), Easing::EaseOut, 30)
    .on("fading in")
    .run(3.0).await;
```

## Time-based animation

`Timeline` schedules keyframed multi-track animation. `AlphaIn`/`AlphaOut` are buffer-modifying alpha tracks. `FadeEnvelope` wraps any effect with fade-in/fade-out bookends:

```rust
FadeEnvelope::new(Plasma::on("hello"))
    .total(8.0)
    .fade_in(1.0, Easing::EaseOut)
    .fade_out(1.0, Easing::EaseIn)
    .run().await;

// DelayedStart holds black for N seconds before the inner effect starts
DelayedStart::new(secs_to_frames(2.0), my_effect).run(10.0).await;
```

## Control

`.spawn()` returns a handle you can command from another task:

```rust
let anim = Plasma::on("loading...").spawn();
// ...do work...
anim.fade_out(1.0);                             // 1s ease to background
anim.fade_out_to(Color::new(0, 0, 0), 0.5);     // 0.5s ease to black
anim.transition_to(Neon::on("done!"), 1.0);     // 1s crossfade
anim.stop();                                     // hard cut
anim.wait().await;                               // wait for finish
```

## Progress bars

indicatif-style with chromakopia palettes:

```rust
let mut bar = ProgressBar::new(1000)
    .width(40)
    .chars("━╸ ")
    .template("{spinner} {bar} {pos}/{len} {msg}")
    .filled_color(Color::new(0, 255, 136))
    .empty_color(Color::new(40, 40, 40));

bar.set_position(420);
bar.set_message("downloading...");
println!("{}", bar.render_template());
```

## Inline rendering

Render frames yourself for embedded use:

```rust
let effect = Rainbow::on("loading...");
for frame in 0..100 {
    print!("\r{}", effect.frame(frame));
    std::thread::sleep(std::time::Duration::from_millis(33));
}
```

## Gradients

Static gradients for non-animated colour:

```rust
use chromakopia::gradient;

println!("{}", gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello!"));
println!("{}", gradient(&["cyan", "pink"]).hsv().apply("Smooth"));
```

18 presets: `rainbow`, `storm`, `passion`, `cristal`, `morning`, `vice`, `atlas`, `teen`, `mind`, `fruit`, `instagram`, `retro`, `summer`, `pastel`, `dark_n_stormy`, `mist`, `relic`, `flughafen`.

## Terminal detection

Auto-detects background/foreground colours for theme-aware animations. Respects the `NO_COLOR` standard and TTY detection — pipes and CI runs fall back to plain text automatically.

```rust
if chromakopia::is_dark_theme() { /* light effects */ }
chromakopia::set_color_enabled(Some(false));  // explicit override
```

## Examples

```sh
cargo run --release --example rainbow   # animated rainbow text
cargo run --release --example plasma    # demoscene plasma
cargo run --release --example demo      # gradient presets
cargo run --release --example license   # composed multi-effect scene
cargo run --release --example marquee   # every Marquee mode, labeled
```

## License

MIT
