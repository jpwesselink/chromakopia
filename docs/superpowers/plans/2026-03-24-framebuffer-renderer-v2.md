# Framebuffer Renderer v0.2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the string-based animation renderer with a framebuffer-based two-loop architecture where effects write `(char, Color)` to a grid and a fixed-rate renderer diffs and flushes to stderr.

**Architecture:** Effects implement an `Effect` trait that writes to a `FrameBuffer` grid. Both the animation task and the render task tick at the same fixed rate (60fps). Animation produces a frame, posts it to a mailbox. Render takes the latest frame, diffs it against the previous, and emits only changed cells via absolute cursor positioning. No overproduction, no frame dropping — one animation frame per render frame. A `Scene` builder provides the declarative API for composing static text, animated effects, transitions, and timelines.

**Tech Stack:** Rust, tokio (async runtime), libc (terminal probing on unix)

**Branch:** `feat/framebuffer-renderer` (already exists with prototype code)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/animate/framebuffer.rs` | Rewrite | Cell, FrameBuffer, diff_render, get_cursor_row, run_effect |
| `src/animate/effects.rs` | Rewrite | Native Effect implementations (all 12 effects) |
| `src/animate/scene.rs` | Create | Scene builder, SceneLine, timeline, transitions |
| `src/animate/easing.rs` | Keep | Easing curves (unchanged) |
| `src/animate/mod.rs` | Rewrite | Public API: standalone animation fns, re-exports |
| `src/lib.rs` | Modify | Update re-exports |
| `src/color.rs` | Keep | Unchanged |
| `src/gradient.rs` | Keep | Unchanged |
| `src/terminal.rs` | Keep | Unchanged |
| `src/presets.rs` | Keep | Unchanged |

---

### Task 1: Clean slate — strip old animation code

**Files:**
- Modify: `src/animate/mod.rs`
- Delete: `src/animate/fb_effects.rs`
- Modify: `src/animate/framebuffer.rs`

Remove the old string-based animation system and the prototype fb_effects. Keep only: framebuffer core (Cell, FrameBuffer, Effect trait, diff_render, run_effect), easing, and the module structure.

- [ ] **Step 1: Gut `src/animate/mod.rs`**

Strip everything except module declarations and re-exports. Remove: Sequence, Animation, spawn_animation, all effect factories, all standalone animation fns, composite, truncate_ansi, render_frame, apply_fade_toward, apply_fade_toward_gradient, FadeTarget, FadeKind, TimeRange. Keep the module structure:

```rust
//! Animated terminal effects — framebuffer-based renderer.

mod effects;
mod easing;
pub mod framebuffer;
mod scene;

pub use easing::Easing;
pub use framebuffer::{Cell, Effect, FrameBuffer, run_effect};
pub use scene::{Scene, SceneLine};
```

- [ ] **Step 2: Delete `src/animate/fb_effects.rs`**

This prototype is being replaced by the rewritten `effects.rs`.

- [ ] **Step 3: Create empty `src/animate/scene.rs`**

```rust
//! Declarative scene builder for framebuffer animations.
```

- [ ] **Step 4: Create empty `src/animate/effects.rs`**

```rust
//! Native framebuffer effect implementations.

use crate::color::Color;
use crate::gradient::Gradient;
use super::framebuffer::{Cell, Effect, FrameBuffer};
```

- [ ] **Step 5: Update `src/lib.rs`**

Strip old re-exports that no longer exist. Keep Color, Gradient, terminal functions, pad, center:

```rust
mod color;
mod gradient;
mod terminal;
pub mod animate;
pub mod presets;

pub use color::Color;
pub use gradient::{Gradient, HsvSpin, Interpolation};
pub use terminal::{bg_color, fg_color, is_dark_theme, is_light_theme, probe_colors, set_bg_color, set_fg_color, terminal_width};

pub fn gradient(colors: &[&str]) -> Gradient { /* unchanged */ }
pub fn pad(text: &str) -> String { /* unchanged */ }
pub fn center(text: &str) -> String { /* unchanged */ }
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build`
Expected: compiles (no examples will work yet)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: strip old animation system, keep framebuffer core"
```

---

### Task 2: Port core effects — rainbow, glow, plasma

**Files:**
- Modify: `src/animate/effects.rs`

Port the three most-used color effects as native `Effect` implementations. Each takes text + config at construction time and writes directly to the FrameBuffer.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_buf(text: &str) -> FrameBuffer {
        FrameBuffer::from_text(text, Color::new(255, 255, 255))
    }

    #[test]
    fn rainbow_changes_colors() {
        let effect = Rainbow::new("hello");
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(0, 0).color;
        effect.render(&mut buf, 10);
        let c1 = buf.get(0, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn rainbow_preserves_chars() {
        let effect = Rainbow::new("hello");
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'h');
        assert_eq!(buf.get(4, 0).ch, 'o');
    }

    #[test]
    fn rainbow_preserves_spaces() {
        let effect = Rainbow::new("a b");
        let mut buf = make_buf("a b");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(1, 0).ch, ' ');
    }

    #[test]
    fn glow_changes_over_time() {
        let pal = vec![Color::new(255, 0, 0), Color::new(0, 0, 255)];
        let effect = Glow::new("hello", pal);
        let mut buf = make_buf("hello");
        effect.render(&mut buf, 0);
        let c0 = buf.get(2, 0).color;
        effect.render(&mut buf, 30);
        let c1 = buf.get(2, 0).color;
        assert_ne!(c0, c1);
    }

    #[test]
    fn plasma_multiline() {
        let effect = Plasma::new("ab\ncd", vec![Color::new(255, 0, 0), Color::new(0, 0, 255)], 0.0);
        let mut buf = make_buf("ab\ncd");
        effect.render(&mut buf, 0);
        assert_eq!(buf.get(0, 0).ch, 'a');
        assert_eq!(buf.get(0, 1).ch, 'c');
    }
}
```

- [ ] **Step 2: Run tests — verify FAIL**

Run: `cargo test`
Expected: FAIL — Rainbow, Glow, Plasma don't exist

- [ ] **Step 3: Implement Rainbow**

```rust
/// Rainbow HSV hue rotation across text.
pub struct Rainbow {
    chars: Vec<Vec<char>>,
}

impl Rainbow {
    pub fn new(text: &str) -> Self {
        Self {
            chars: text.split('\n').map(|l| l.chars().collect()).collect(),
        }
    }
}

impl Effect for Rainbow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let hue_offset = (frame * 5 % 360) as f64;
        let max_width = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let hue = (hue_offset + (x as f64 / max_width as f64) * 360.0) % 360.0;
                let color = Color::from_hsv(hue, 1.0, 1.0);
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}
```

- [ ] **Step 4: Implement Glow**

```rust
/// Sweeping spotlight that travels through a gradient palette.
pub struct Glow {
    chars: Vec<Vec<char>>,
    palette: Vec<Color>,
}

impl Glow {
    pub fn new(text: &str, palette: Vec<Color>) -> Self {
        Self {
            chars: text.split('\n').map(|l| l.chars().collect()).collect(),
            palette,
        }
    }
}

impl Effect for Glow {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let max_width = self.chars.iter().map(|l| l.len()).max().unwrap_or(1).max(1);
        let pal = &self.palette;
        if pal.is_empty() { return; }

        let spotlight = (frame as f64 * 0.02).sin() * 0.5 + 0.5; // 0..1 sweep

        for (y, line) in self.chars.iter().enumerate() {
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width || y >= buf.height { continue; }
                let pos = x as f64 / max_width as f64;
                // Bright near spotlight, dim far away
                let dist = (pos - spotlight).abs();
                let brightness = (1.0 - dist * 3.0).max(0.15);

                let idx = (pos * (pal.len() - 1) as f64).min((pal.len() - 1) as f64);
                let lo = idx.floor() as usize;
                let hi = (lo + 1).min(pal.len() - 1);
                let frac = idx - lo as f64;
                let base = Color::lerp_rgb(pal[lo], pal[hi], frac);
                let color = Color::new(
                    (base.r as f64 * brightness) as u8,
                    (base.g as f64 * brightness) as u8,
                    (base.b as f64 * brightness) as u8,
                );
                buf.set(x, y, Cell::new(ch, color));
            }
        }
    }
}
```

- [ ] **Step 5: Implement Plasma** (copy from existing fb_effects.rs)

- [ ] **Step 6: Run tests — verify PASS**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add src/animate/effects.rs
git commit -m "feat: native framebuffer effects — Rainbow, Glow, Plasma"
```

---

### Task 3: Port remaining effects — pulse, glitch, radar, neon, karaoke, flap, sparkle

**Files:**
- Modify: `src/animate/effects.rs`

Port the remaining 7 color/character effects. These are simpler — mostly per-character color math.

- [ ] **Step 1: Write tests for each effect**

One test per effect verifying: chars preserved, colors change over frames.

- [ ] **Step 2: Run tests — verify FAIL**

- [ ] **Step 3: Implement all 7 effects**

Each follows the same pattern: struct with `chars: Vec<Vec<char>>` + config, `Effect` impl that iterates the grid.

Key effects:
- `Pulse` — red highlight expanding/contracting from center
- `Glitch` — random character corruption (uses rand)
- `Radar` — spotlight sweep (angular, not linear like Glow)
- `Neon` — alternating dim/bright per frame
- `Karaoke` — progressive character reveal
- `Flap` — split-flap board letter randomization
- `Sparkle` — radial twinkling (copy from existing)

- [ ] **Step 4: Run tests — verify PASS**

- [ ] **Step 5: Commit**

```bash
git add src/animate/effects.rs
git commit -m "feat: port pulse, glitch, radar, neon, karaoke, flap, sparkle to framebuffer"
```

---

### Task 4: Scene builder — declarative composition

**Files:**
- Create: `src/animate/scene.rs`

The Scene is the primary user-facing API. Replaces Sequence.

- [ ] **Step 1: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn scene_dimensions() {
        let scene = Scene::new()
            .line(SceneLine::Static("hello", Color::new(255, 255, 255)))
            .line(SceneLine::Blank)
            .line(SceneLine::Static("world", Color::new(255, 255, 255)));
        assert_eq!(scene.width(), 5);
        assert_eq!(scene.height(), 3);
    }

    #[test]
    fn scene_text_block() {
        let scene = Scene::new()
            .text_block("ab\ncd", |l| SceneLine::Static(l, Color::new(255, 255, 255)));
        assert_eq!(scene.height(), 2);
    }
}
```

- [ ] **Step 2: Implement Scene and SceneLine**

```rust
pub enum SceneLine {
    Static(String, Color),
    Animated(String, Box<dyn Fn() -> Box<dyn Effect>>),
    Blank,
}

pub struct Scene {
    lines: Vec<SceneLine>,
    seed: f64,
}
```

Key change from prototype: `SceneLine::Animated` takes a factory closure that produces an `Effect` for that line's text. This avoids the `&'static str` / transmute hack.

Scene methods: `new()`, `seed()`, `line()`, `text_block()`, `width()`, `height()`, `run()`.

`run()` builds a `SceneEffect` that implements `Effect` and passes it to `run_effect()`.

- [ ] **Step 3: Run tests — verify PASS**

- [ ] **Step 4: Commit**

```bash
git add src/animate/scene.rs
git commit -m "feat: Scene builder with static, animated, and blank lines"
```

---

### Task 5: Standalone animation API

**Files:**
- Modify: `src/animate/mod.rs`

Rebuild the simple `animate::rainbow(text, speed)` etc functions that users expect. These create a single-effect Scene and run it as a background task with start/stop control.

- [ ] **Step 1: Implement Animation struct + standalone functions**

```rust
pub struct Animation { /* running flag, handle */ }

impl Animation {
    pub fn stop(&self) { /* set running=false */ }
}

pub fn rainbow(text: &str, speed: f64) -> Animation { /* spawn scene */ }
pub fn plasma(text: &str, speed: f64) -> Animation { /* spawn scene */ }
pub fn glow(grad: Gradient, text: &str, speed: f64) -> Animation { /* spawn scene */ }
// ... etc for all effects
```

- [ ] **Step 2: Write a test**

```rust
#[tokio::test]
async fn animation_starts_and_stops() {
    let anim = rainbow("test", 1.0);
    tokio::time::sleep(Duration::from_millis(100)).await;
    anim.stop();
}
```

- [ ] **Step 3: Run tests — verify PASS**

- [ ] **Step 4: Commit**

```bash
git add src/animate/mod.rs
git commit -m "feat: standalone animation API (rainbow, plasma, glow, etc)"
```

---

### Task 6: Update examples

**Files:**
- Modify: all examples in `examples/`

Update examples to use the new API. Most will get simpler since Scene handles layout.

- [ ] **Step 1: Update simple examples** (rainbow, pulse, glitch, radar, neon, karaoke, flap, glow, cycle)

These should work with the standalone API unchanged: `animate::rainbow("text", 1.0)`.

- [ ] **Step 2: Update Scene examples** (fb_demo, license, showcase, scroll)

Use the Scene builder.

- [ ] **Step 3: Update static examples** (demo, theme_adaptive, starfield)

- [ ] **Step 4: Verify all examples build**

Run: `cargo build --examples`

- [ ] **Step 5: Commit**

```bash
git add examples/
git commit -m "feat: update all examples for framebuffer renderer"
```

---

### Task 7: Final cleanup and verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`

- [ ] **Step 3: Build all examples**

Run: `cargo build --examples`

- [ ] **Step 4: Update README**

Update the README to document:
- Scene API (replaces Sequence)
- New effects list
- Updated example list

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: cleanup, clippy, update docs for v0.2"
```
