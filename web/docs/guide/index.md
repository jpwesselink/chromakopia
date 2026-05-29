---
title: Getting Started
---

# Getting Started

## Installation

```toml
[dependencies]
chromakopia = "0.2"
tokio = { version = "1", features = ["full"] }
```

## Quick start

```rust
use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    // Animate text for 3 seconds, then fade out
    let anim = Rainbow::on("Hello, world!").spawn();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    anim.fade_out(1.0);
    anim.wait().await;
}
```

## Effects

Color effects are zero-config. Use `.on("text")` to give them text:

```rust
// Simple effects
Rainbow::on("hello");
Neon::on("blink");
Glow::new().palette(presets::mist().palette(256)).on("glow");
Plasma::on("fire").palette(presets::storm().palette(256));

// Compose: chain effects over time
Chain::new()
    .then(3.0, Rainbow::on("first"))
    .then(3.0, Transition::new(Rainbow::on("first"), Neon::on("second"), 2.0, Easing::EaseInOut))
    .then(3.0, Neon::on("second"));
```

## Scenes

Stack multiple effects vertically:

```rust
Scene::new()
    .add(text("MIT License", Color::new(255, 255, 255)))
    .blank()
    .add(Rainbow::on("colored text"))
    .add(Plasma::on("more text").palette(presets::storm().palette(256)))
    .run(5.0)
    .await;
```

## AnimationHandle

Control running animations:

```rust
let anim = Plasma::on("loading...").spawn();

// Later — crossfade to a new effect
anim.transition_to(Glow::on("done!"), 1.0);
anim.wait().await;

// Or fade out
anim.fade_out(1.0);
anim.wait().await;
```

## FadeEnvelope

Wrap any effect with fade-in and fade-out:

```rust
FadeEnvelope::new(Rainbow::on("text"))
    .total(10.0)           // total duration
    .fade_in(1.0, Easing::EaseOut)
    .fade_out(2.0, Easing::EaseInOut)
    .from_color(bg)        // start/end color
```

## ProgressBar

indicatif-style progress bar with templates, spinners, and color effects:

```rust
use chromakopia::prelude::*;

// Template with all the bells and whistles
let mut bar = ProgressBar::new(1000)
    .width(30)
    .chars("█▓░")
    .template("{prefix} {bar} {pos}/{len} ({percent}%) {eta} {msg}")
    .filled_color(Color::new(0, 255, 136))
    .empty_color(Color::new(40, 40, 40));

bar.set_prefix("[dl]");
bar.set_message("fetching...");
bar.set_position(420);
print!("\r{}", bar.render_template());

bar.inc(1);
bar.finish_with_message("done!");
```

Spinners for indeterminate progress:

```rust
let mut spin = ProgressBar::spinner()
    .template("{spinner} {msg}")
    .spinner_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏");
spin.set_message("compiling...");
spin.tick();
print!("\r{}", spin.render_template());
```

Pipe through any effect for animated color:

```rust
let bar = ProgressBar::new(100).width(40).chars("█░");
let text = bar.text(0.7);

// Rainbow-colored progress bar
print!("\r{}", Rainbow::on(&text).frame(tick));

// Plasma fire progress bar
let fire = presets::storm().palette(256);
print!("\r{}", Plasma::new().palette(fire).on(&text).frame(tick));
```

Template placeholders: `{bar}`, `{bar:WIDTH}`, `{pos}`, `{len}`, `{percent}`, `{msg}`, `{prefix}`, `{spinner}`, `{elapsed}`, `{eta}`.

## DYCP

Per-character vertical sine wave — classic demoscene effect:

```rust
Dycp::new("scrolling text here")
    .amplitude(4.0)
    .frequency(0.10)
    .speed(0.08)
    .scroll(0.2)                    // horizontal scroll speed
    .scroll_in(-(term_width as i64)) // start off-screen right
    .ease_in(120)                    // ease to rest over 120 frames
    .color(Rainbow::new())           // any effect for coloring
    .frame(tick)
```

## FLD

Flexible Line Distance — shifts entire scanlines vertically:

```rust
// Wrap any effect — the rendered output ripples like a rubber sheet
let fld = Fld::new(Rainbow::on("hello world"))
    .amplitude(2.0)
    .frequency(0.0)   // 0 = uniform shift (whole block bounces)
    .speed(0.1)
    .delay(90)         // wait 90 frames before starting
    .ramp(30);         // ramp amplitude over 30 frames
```

Combine with DYCP for layered demoscene effects:

```rust
// Characters bounce (DYCP), then the whole block floats (FLD)
let effect = Fld::new(
    Dycp::new("chromakopia")
        .amplitude(3.0)
        .color(Plasma::new().palette(fire))
).amplitude(2.0).frequency(0.0).speed(0.12);
```

## Presets

Built-in gradient palettes:

```rust
presets::storm()         // deep indigo → electric purple → hot orange → gold
presets::mist()          // pale blue-white → pine green
presets::flughafen()     // bright amber → warm gold
presets::starfield()     // void black → electric blue → white hot → violet
```
