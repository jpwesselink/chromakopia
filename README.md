# chromakopia

Terminal animation engine for Rust.

```rust
use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    Rainbow::on("Hello, world!").run(3.0).await;
}
```

## Install

```toml
[dependencies]
chromakopia = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

## Effects

Built-in effects. `.on("text")` gives an effect its text:

```rust
Rainbow::on("text").spawn();       // HSV hue rotation
Plasma::on("text").spawn();        // demoscene sine waves
Neon::on("text").spawn();          // flickering bright/dim
Pulse::on("text").spawn();         // expanding red highlight
Radar::on("text").spawn();         // sweeping spotlight
Glow::on("text").spawn();          // gradient spotlight
Karaoke::on("text").spawn();       // progressive reveal
```

Customize with builders — `.on()` works anywhere in the chain:

```rust
Plasma::on("text").palette(presets::storm().palette(256)).seed(42.0);
Plasma::new().palette(presets::storm().palette(256)).on("text");
// both work
```

## Scenes

Stack effects vertically with `Scene`:

```rust
let white = Color::new(255, 255, 255);

Scene::new()
    .add(text("MIT License", white))
    .blank()
    .add(Rainbow::on("colored heading"))
    .add(Plasma::on("body text\nwith multiple lines"))
    .run(5.0)
    .await;
```

## Control

`.spawn()` returns a handle you can command:

```rust
let anim = Plasma::on("loading...").spawn();

// ...do work...

anim.fade_out(1.0);                           // 1s ease to background
anim.fade_out_to(Color::new(0, 0, 0), 0.5);  // 0.5s ease to black
anim.transition_to(Neon::on("done!"), 1.0);   // 1s crossfade
anim.stop();                                   // hard cut
anim.wait().await;                             // wait for finish
```

## Inline

Render frames yourself for progress bars, spinners, embedded use:

```rust
let effect = Rainbow::on("loading...");
for frame in 0..100 {
    print!("\r{}", effect.frame(frame));
    std::thread::sleep(std::time::Duration::from_millis(33));
}
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

## Gradients

Static gradients for non-animated color:

```rust
use chromakopia::gradient;

println!("{}", gradient(&["#ff0000", "#00ff00", "#0000ff"]).apply("Hello!"));
println!("{}", gradient(&["cyan", "pink"]).hsv().apply("Smooth"));
```

18 presets: `rainbow`, `storm`, `passion`, `cristal`, `morning`, `vice`, `atlas`, `teen`, `mind`, `fruit`, `instagram`, `retro`, `summer`, `pastel`, `dark_n_stormy`, `mist`, `relic`, `flughafen`.

## Terminal detection

Auto-detects background/foreground colors for theme-aware animations:

```rust
if chromakopia::is_dark_theme() {
    // light effects
} else {
    // dark effects
}
```

## Examples

```sh
cargo run --example rainbow    # animated rainbow text
cargo run --example plasma     # demoscene plasma effect
cargo run --example demo       # static gradient presets
cargo run --example license    # composed multi-effect scene
cargo run --example fb_demo    # DYCP banner + scrolling license
```

## License

MIT
