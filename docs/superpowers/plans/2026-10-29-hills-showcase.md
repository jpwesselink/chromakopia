# Hills Showcase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `examples/hills` into an endless, keyboard-skippable cracktro showcase that presents every chromakopia effect on a styled panel over the pastel landscape.

**Architecture:** Two passes per frame: `Layers` paints the half-block pixel world (landscape, wordmark, scroller, panel frame, captions), then the active cell effect renders into a sub-buffer that is blitted onto the panel interior. A pure, tested `stage` module turns wall time plus seek requests into "which step, how far in". The library gains `Key`, `parse_keys` and `Scene::on_key`.

**Tech Stack:** Rust 2021, tokio, chromakopia `Layers`/`Canvas`/`Scene`, figlet `.flf` fonts in `examples/fonts`, C64 character ROM bytes.

**Spec:** `docs/superpowers/specs/2026-10-29-hills-showcase-design.md`

## Global Constraints

- Frame budget: every block renders well under 16.7 ms/frame at 200 columns (60 fps).
- Panel fill `#2A1638` (aubergine, = `LETTER_OUTLINE`); panel extrusion = two steps of `LETTER_EXTRUDE` down-right, as on the letters.
- Panel y is always a whole terminal cell.
- Keys: `→`/`n`/space next panel, `←`/`p` previous, `↓` next block, `↑` previous block, `1`–`6` jump to block, `q` quit.
- Loop: after the finale the show continues at the logo exit, then block 2.
- `examples/greetings.rs` is not modified (its figlet loader is copied, not moved).
- Run the demo with `Scene::fullscreen()` (already implemented) so quitting restores the terminal.

## Review Focus

- **Narrow terminal (80 columns or less):** panels clamp to the screen and long names fall back from Colossal to Basic; nothing panics. Test in Task 5 (`fit_falls_back_to_basic`) and Task 8 (`geometry_clamps_to_narrow_screen`).
- **Short terminal (fewer than 40 rows):** the buffer is smaller than the scene; blitting and painting clip instead of panicking. Test in Task 8 (`blit_clips_to_small_buffer`).
- **Seeking past either end:** `←` at the very start stays on the intro; `→` on the finale wraps to the loop target. Test in Task 6 (`prev_at_start_stays`, `next_on_finale_loops`).
- **Out-of-range block jump:** `Block(0)`/`Block(9)` clamp to the first/last block instead of panicking. Test in Task 6 (`block_jump_clamps`).
- **Several keys in one frame:** the last request wins and is applied exactly once. Test in Task 6 (`clock_applies_seek_once`).

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src/animate/keys.rs` (new) | `Key`, `parse_keys`, `KeyHandler` type |
| `src/animate/framebuffer.rs` | key listener forwards keys to an `on_key` handler |
| `src/animate/scene.rs` | `Scene::on_key` |
| `examples/hills/main.rs` (moved from `examples/hills.rs`) | `ROWS`, `TOP_PAD`, `View`, `Stage`, wiring, keys |
| `examples/hills/landscape.rs` | palette, ridges, sky, cloud shadows → `landscape() -> Layers` |
| `examples/hills/glyphs.rs` | letter colours, Mario + C64 glyph tables, `stamp`, `stamp_text` |
| `examples/hills/wordmark.rs` | CHROMAKOPIA wordmark, enter/exit/DYCP motion |
| `examples/hills/scroller.rs` | licence scroller |
| `examples/hills/figlet.rs` | `.flf` loader, `fit()` |
| `examples/hills/stage.rs` | storyboard, `locate`, `Seek`, `seek_target`, `Clock` |
| `examples/hills/panel.rs` | panel geometry, motion, frame painting, effect factory, blit |

(The spec lists five example files; `wordmark.rs`, `scroller.rs` and `panel.rs` split them further so each file has one job.)

---

### Task 1: `Key` and `parse_keys`

**Files:**
- Create: `src/animate/keys.rs`
- Modify: `src/animate/mod.rs`, `src/lib.rs` (prelude)

**Interfaces:**
- Produces: `pub enum Key { Char(char), Left, Right, Up, Down }` (Copy, Eq, Debug); `pub fn parse_keys(bytes: &[u8]) -> Vec<Key>`; `pub(crate) type KeyHandler = std::sync::Arc<dyn Fn(Key) + Send + Sync>`.

- [ ] **Step 1: Write the failing tests**

Create `src/animate/keys.rs` with only the tests (plus `use super::*` target that does not exist yet):

```rust
//! Keyboard input for interactive animations.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_chars() {
        assert_eq!(parse_keys(b"n 1q"), vec![Key::Char('n'), Key::Char(' '), Key::Char('1'), Key::Char('q')]);
    }

    #[test]
    fn arrows_csi_and_ss3() {
        assert_eq!(
            parse_keys(b"\x1B[A\x1B[B\x1B[C\x1B[D\x1BOC"),
            vec![Key::Up, Key::Down, Key::Right, Key::Left, Key::Right]
        );
    }

    #[test]
    fn mixed_input() {
        assert_eq!(parse_keys(b"a\x1B[Cb"), vec![Key::Char('a'), Key::Right, Key::Char('b')]);
    }

    #[test]
    fn unknown_escapes_are_dropped_whole() {
        // Delete key, Ctrl-Right, a lone ESC.
        assert_eq!(parse_keys(b"\x1B[3~x\x1B[1;5Cy\x1B"), vec![Key::Char('x'), Key::Char('y')]);
    }

    #[test]
    fn control_and_non_ascii_bytes_are_skipped() {
        assert_eq!(parse_keys(&[0x03, 0xC3, 0xA9, b'z']), vec![Key::Char('z')]);
    }
}
```

Add `mod keys;` to `src/animate/mod.rs` (next to `mod layers;`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib keys`
Expected: FAIL to compile — `cannot find type Key` / `cannot find function parse_keys`.

- [ ] **Step 3: Implement**

Insert above the tests in `src/animate/keys.rs`:

```rust
use std::sync::Arc;

/// A key press, as delivered to `Scene::on_key` handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Left,
    Right,
    Up,
    Down,
}

/// Shared key callback, called from the input thread.
pub(crate) type KeyHandler = Arc<dyn Fn(Key) + Send + Sync>;

/// Decode raw terminal bytes into keys. Arrows arrive as `ESC [ A–D`
/// (or `ESC O A–D` in application mode); any other escape sequence is
/// dropped whole. Control bytes and non-ASCII bytes are skipped.
pub fn parse_keys(bytes: &[u8]) -> Vec<Key> {
    let mut keys = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1B {
            if i + 2 < bytes.len() && matches!(bytes[i + 1], b'[' | b'O') {
                let arrow = match bytes[i + 2] {
                    b'A' => Some(Key::Up),
                    b'B' => Some(Key::Down),
                    b'C' => Some(Key::Right),
                    b'D' => Some(Key::Left),
                    _ => None,
                };
                if let Some(key) = arrow {
                    keys.push(key);
                    i += 3;
                } else {
                    // Unknown sequence: skip through its final byte (0x40–0x7E).
                    let mut j = i + 2;
                    while j < bytes.len() && !(0x40..=0x7E).contains(&bytes[j]) {
                        j += 1;
                    }
                    i = j + 1;
                }
            } else {
                i += 1; // lone ESC
            }
            continue;
        }
        if (0x20..0x7F).contains(&b) {
            keys.push(Key::Char(b as char));
        }
        i += 1;
    }
    keys
}
```

Exports — `src/animate/mod.rs`:

```rust
pub use keys::{parse_keys, Key};
pub(crate) use keys::KeyHandler;
```

`src/lib.rs` prelude: append `Key,` after `Canvas,` in the `pub use crate::animate::{ … }` list.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib keys`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/animate/keys.rs src/animate/mod.rs src/lib.rs
git commit -m "feat: Key and parse_keys for terminal key input"
```

---

### Task 2: Key listener + `Scene::on_key`

**Files:**
- Modify: `src/animate/framebuffer.rs` (`spawn_quit_listener` and its two call sites, `run_effect`, `run_effect_in`)
- Modify: `src/animate/scene.rs` (`Scene` struct, `new`, `on_key`, `run`, tests)

**Interfaces:**
- Consumes: `Key`, `parse_keys`, `KeyHandler` (Task 1).
- Produces: `Scene::on_key(self, f: impl Fn(Key) + Send + Sync + 'static) -> Self` — handler runs on the input thread for every key except the quit key; only honoured by `Scene::run`.

- [ ] **Step 1: Write the failing test** (append inside `mod tests` in `src/animate/scene.rs`)

```rust
    #[test]
    fn on_key_handler_receives_keys() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let scene = Scene::new().on_key(move |k| {
            if k == super::super::keys::Key::Right {
                h.fetch_add(1, Ordering::SeqCst);
            }
        });
        let handler = scene.on_key.as_ref().expect("handler stored");
        handler(super::super::keys::Key::Right);
        handler(super::super::keys::Key::Left);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib on_key_handler_receives_keys`
Expected: FAIL — `no method named on_key` / `no field on_key`.

- [ ] **Step 3: Implement in `scene.rs`**

Add the import at the top: `use super::keys::{Key, KeyHandler};`

Struct field and constructor:

```rust
pub struct Scene {
    items: Vec<SceneItem>,
    quit_key: Option<char>,
    fullscreen: bool,
    on_key: Option<KeyHandler>,
}
```

```rust
        Self { items: Vec::new(), quit_key: None, fullscreen: false, on_key: None }
```

Builder (after `quit_key`):

```rust
    /// Call `f` for every key pressed while the scene runs (except the
    /// quit key, which still stops it). Runs on the input thread — keep it
    /// short; hand work to the effect through a shared value. Honoured by
    /// `run`.
    pub fn on_key(mut self, f: impl Fn(Key) + Send + Sync + 'static) -> Self {
        self.on_key = Some(std::sync::Arc::new(f));
        self
    }
```

In `run`, replace the two lines that read the fields and call `run_effect_in` with:

```rust
        let (quit_key, fullscreen, on_key) = (self.quit_key, self.fullscreen, self.on_key.clone());
        super::framebuffer::run_effect_in(self, width, height, Duration::from_secs_f64(seconds), 1.0, quit_key, on_key, fullscreen).await;
```

- [ ] **Step 4: Implement in `framebuffer.rs`**

Add `use super::keys::{parse_keys, Key, KeyHandler};` near the other `use super::` lines (inside the non-wasm cfg if the file groups them that way).

Replace the whole unix `spawn_quit_listener` function with:

```rust
/// Watch `/dev/tty` for keys. `quit` (case-insensitive) flips `running` to
/// false; every other key goes to `on_key`. Clears only ICANON|ECHO — ISIG
/// stays on so Ctrl-C still works. Restores the original termios when the
/// thread exits. Returns a flag that turns true once quit was pressed.
#[cfg(unix)]
fn spawn_key_listener(
    quit: Option<char>,
    on_key: Option<KeyHandler>,
    running: Arc<std::sync::atomic::AtomicBool>,
) -> Arc<std::sync::atomic::AtomicBool> {
    use std::io::Read;
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::{AtomicBool, Ordering};

    let pressed = Arc::new(AtomicBool::new(false));
    let flag = pressed.clone();
    std::thread::spawn(move || {
        let Ok(mut tty) = std::fs::OpenOptions::new().read(true).open("/dev/tty") else {
            return;
        };
        let fd = tty.as_raw_fd();

        let old = unsafe {
            let mut t = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut t) != 0 {
                return;
            }
            t
        };

        struct TermiosGuard {
            fd: i32,
            original: libc::termios,
        }
        impl Drop for TermiosGuard {
            fn drop(&mut self) {
                unsafe {
                    libc::tcsetattr(self.fd, libc::TCSANOW, &self.original);
                }
            }
        }
        let _guard = TermiosGuard { fd, original: old };

        let mut raw = old;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1; // read() returns after ≤100ms — lets us poll `running`
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return;
        }

        let mut bytes = [0u8; 32];
        'read: while running.load(Ordering::Relaxed) {
            match tty.read(&mut bytes) {
                Ok(0) => {}
                Ok(n) => {
                    for key in parse_keys(&bytes[..n]) {
                        if let (Key::Char(c), Some(q)) = (key, quit) {
                            if c.eq_ignore_ascii_case(&q) {
                                flag.store(true, Ordering::Relaxed);
                                running.store(false, Ordering::Relaxed);
                                break 'read;
                            }
                        }
                        if let Some(handler) = &on_key {
                            handler(key);
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    pressed
}
```

Replace the non-unix stub with:

```rust
#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn spawn_key_listener(
    _quit: Option<char>,
    _on_key: Option<KeyHandler>,
    _running: Arc<std::sync::atomic::AtomicBool>,
) -> Arc<std::sync::atomic::AtomicBool> {
    Arc::new(std::sync::atomic::AtomicBool::new(false))
}
```

`run_effect` passes `None` for the handler:

```rust
    run_effect_in(effect, width, height, duration, speed, quit_key, None, false).await
```

`run_effect_in` gains `on_key: Option<KeyHandler>,` as the parameter right after `quit_key: Option<char>,`, and its listener start becomes:

```rust
    let quit_pressed = (quit_key.is_some() || on_key.is_some()).then(|| {
        // Force the OSC color probe now, while we still own the tty.
        crate::terminal::probe_colors();
        spawn_key_listener(quit_key, on_key, running.clone())
    });
```

In `spawn_effect`, the equivalent block becomes:

```rust
    let quit_pressed = quit_key.map(|key| {
        // Force the OSC color probe now, while we still own the tty.
        crate::terminal::probe_colors();
        spawn_key_listener(Some(key), None, running.clone())
    });
```

- [ ] **Step 5: Run tests and build everything**

Run: `cargo test --lib && cargo build --release --examples`
Expected: all lib tests pass (including `on_key_handler_receives_keys`); examples build; the only warning is the pre-existing `LOGO_BLOCK_ART` one in `wind.rs`.

- [ ] **Step 6: Commit**

```bash
git add src/animate/framebuffer.rs src/animate/scene.rs
git commit -m "feat: Scene::on_key — forward key presses to a handler"
```

---

### Task 3: Split `hills.rs` into a directory (pure refactor)

**Files:**
- Move: `examples/hills.rs` → `examples/hills/main.rs`
- Create: `examples/hills/landscape.rs`, `examples/hills/glyphs.rs`, `examples/hills/wordmark.rs`, `examples/hills/scroller.rs`

**Interfaces:**
- Produces (used by later tasks):
  - `main.rs`: `pub const ROWS: usize = 40; pub const TOP_PAD: usize = 6;`, `fn build_layers(screen_w: usize) -> Layers` (landscape + wordmark + scroller, as today).
  - `landscape::landscape() -> Layers`.
  - `glyphs::{LETTER_CREAM, LETTER_PINK, LETTER_LAVENDER, LETTER_OUTLINE, LETTER_EXTRUDE}`, `glyphs::stamp(c: &mut Canvas, glyph: &[u8; 8], at: (i64, i64), scale: i64, extrude: i64, face: &mut Vec<(i64, i64, Color)>)`, `glyphs::mario(ch: char) -> [u8; 8]`, `glyphs::c64(ch: char) -> [u8; 8]`.
  - `wordmark::{Wordmark, WORD}`, `Wordmark::placed(screen_w: usize) -> (Wordmark, i64, i64)`, `Wordmark::paint(&self, c: &mut Canvas, x0: i64, y0: i64)` (unchanged behaviour in this task).
  - `scroller::{Scroller, LICENSE_TEXT}`, `Scroller::new(text: &str, y0: i64)`, `Scroller::paint(&self, c: &mut Canvas)`.

- [ ] **Step 1: Pin today's output with a hash test** (still in `examples/hills.rs`)

Extract the layer building out of `main` into `fn build_layers(screen_w: usize) -> Layers` (body = everything in `main` from `// Landscape fills…` through the scroller layer, with `let screen_w = terminal_width();` removed and `screen_w` taken from the parameter; return `layers`). `main` becomes:

```rust
#[tokio::main]
async fn main() {
    Scene::new()
        .add(build_layers(terminal_width()))
        .fullscreen()
        .quit_key('q')
        .run(3600.0)
        .await;
}
```

Append:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Hash of a few rendered frames — identical before and after a
    /// refactor means the refactor changed nothing on screen.
    fn frames_hash() -> u64 {
        use std::hash::{Hash, Hasher};
        let layers = build_layers(120);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for f in [0usize, 120, 260, 900] {
            let mut b = FrameBuffer::new(120, ROWS);
            layers.render(&mut b, f);
            b.to_ansi_string().hash(&mut h);
        }
        h.finish()
    }

    #[test]
    fn print_frames_hash() {
        println!("frames hash: {}", frames_hash());
    }
}
```

Run: `cargo test --example hills print_frames_hash -- --nocapture`
Expected: PASS, prints `frames hash: <N>`. Write `<N>` down.

- [ ] **Step 2: Move the file**

```bash
mkdir -p examples/hills
git mv examples/hills.rs examples/hills/main.rs
```

- [ ] **Step 3: Distribute items into modules (move verbatim, add `pub` where another module uses an item)**

- `landscape.rs`: header `use chromakopia::animate::dither; use chromakopia::prelude::*; use std::f64::consts::PI; use super::{ROWS, TOP_PAD};`. Move: `SHADOW_BLUE`, `SKY` … `ROSE` (landscape palette), `PERIOD`, `SWING`, `DEPTH`, `LAG`, `ridge`, `ridge_lifted`, `cumulus`. Add `pub fn landscape() -> Layers` whose body is the part of `build_layers` from `// Landscape fills…` up to and including `let layers = layers.layer(shadows);`, returning `layers`.
- `glyphs.rs`: header `use chromakopia::prelude::*;`. Move `LETTER_*` consts (make `pub`), `MARIO_GLYPHS`, `C64_GLYPHS`, `stamp` (make `pub`). Add:

```rust
/// A Mario-style capital as an 8×8 bitmask (MSB = leftmost pixel).
pub fn mario(ch: char) -> [u8; 8] {
    let rows = MARIO_GLYPHS.iter().find(|(g, _)| *g == ch).expect("glyph in MARIO_GLYPHS").1;
    rows.map(|row| row.bytes().enumerate().fold(0, |m, (i, b)| if b == b'#' { m | 0x80 >> i } else { m }))
}

/// A C64 character-ROM glyph as an 8×8 bitmask.
pub fn c64(ch: char) -> [u8; 8] {
    C64_GLYPHS.iter().find(|(g, _)| *g == ch).expect("glyph in C64_GLYPHS").1
}
```

- `wordmark.rs`: header `use chromakopia::animate::framebuffer::FPS; use chromakopia::prelude::*; use std::f64::consts::TAU; use super::glyphs::{mario, stamp}; use super::ROWS;`. Move `WORD` (pub), `LOGO_LIFT`, `DYCP_*`, `DROP_*`, `FALL`, `DYCP_RAMP`, `ADVANCE`, `struct Bitmap` + impl, renamed to `pub struct Wordmark`. `Wordmark::new` uses `mario(c)` instead of the inline lookup. Add:

```rust
    /// The wordmark sized and placed for a screen `screen_w` columns wide:
    /// (wordmark, x0, y0).
    pub fn placed(screen_w: usize) -> (Wordmark, i64, i64) {
        let word = Wordmark::new(WORD, (screen_w / (WORD.len() * ADVANCE + 8)).clamp(1, 3));
        let x0 = (screen_w as i64 - word.width() as i64) / 2;
        // Centred, then lifted LOGO_LIFT pixels — but never so high that the
        // tallest DYCP swing (plus outline) leaves the top of the screen.
        let y0 = ((ROWS * 2) as i64 / 2 - word.height() as i64 / 2 - LOGO_LIFT).max(word.max_swing() + 3);
        (word, x0, y0)
    }
```

- `scroller.rs`: header `use chromakopia::animate::framebuffer::FPS; use chromakopia::prelude::*; use std::f64::consts::TAU; use super::glyphs::{c64, stamp};`. Move `SCROLL_*`, `Scroller` (pub) + impl (`new` uses `c64(c)`), `LICENSE_TEXT` (pub).
- `main.rs` keeps: doc comment, `mod glyphs; mod landscape; mod scroller; mod wordmark;`, `use chromakopia::prelude::*;`, `pub const ROWS`, `pub const TOP_PAD`, `build_layers`, `main`, tests. `build_layers` becomes:

```rust
fn build_layers(screen_w: usize) -> Layers {
    let (word, x0, y0) = wordmark::Wordmark::placed(screen_w);
    // Baseline high enough that the scroller's wave plus extrusion stays on screen.
    let scroller = scroller::Scroller::new(scroller::LICENSE_TEXT, (ROWS * 2) as i64 - 15);
    landscape::landscape()
        .layer(Layer::paint(move |c| word.paint(c, x0, y0)))
        .layer(Layer::paint(move |c| scroller.paint(c)))
}
```

- [ ] **Step 4: Verify the hash is unchanged**

Run: `cargo test --example hills print_frames_hash -- --nocapture`
Expected: PASS with the same `frames hash: <N>` as Step 1. Also `cargo build --release --example hills` with no warnings from `examples/hills`.

- [ ] **Step 5: Replace the print test with a smoke test**

Replace `print_frames_hash` with:

```rust
    #[test]
    fn layers_render_without_panicking() {
        let layers = build_layers(120);
        let mut b = FrameBuffer::new(120, ROWS);
        layers.render(&mut b, 260);
        assert!(b.to_ansi_string().contains('▀'));
    }
```

(`frames_hash` goes too.) Run `cargo test --example hills` → PASS.

- [ ] **Step 6: Commit**

```bash
git add examples/hills
git commit -m "refactor: split hills example into modules"
```

---

### Task 4: Full C64 caption font + `stamp_text`

**Files:**
- Modify: `examples/hills/glyphs.rs`

**Interfaces:**
- Consumes: `stamp`, `c64` (Task 3).
- Produces: `glyphs::has_c64(ch: char) -> bool`; `glyphs::text_width(text: &str) -> i64` (= chars × 8); `glyphs::stamp_text(c: &mut Canvas, text: &str, at: (i64, i64), face: &mut Vec<(i64, i64, Color)>)` (scale 1, one extrusion step, 8-pixel advance).

- [ ] **Step 1: Write the failing tests** (append to `glyphs.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_upper_and_lower_case() {
        assert_eq!(c64('A'), [0x18, 0x3C, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00]);
        assert_eq!(c64('a'), [0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00]);
    }

    #[test]
    fn caption_punctuation_present() {
        for ch in "&:-,.'!?".chars() {
            assert!(has_c64(ch), "missing {ch:?}");
        }
        assert!(!has_c64('×'));
    }

    #[test]
    fn text_width_is_eight_per_char() {
        assert_eq!(text_width("COLOR"), 40);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills glyphs`
Expected: FAIL to compile — `has_c64`, `text_width` not found.

- [ ] **Step 3: Regenerate `C64_GLYPHS` with every printable character**

Write this script to the scratchpad (not the repo) as `gen_c64.py` and run it with `python3 -I gen_c64.py > c64_table.rs`:

```python
import urllib.request

ROM = "https://raw.githubusercontent.com/cbmeeks/VgaPico/HEAD/fonts/petscii/chargen"
rom = urllib.request.urlopen(ROM).read()
assert len(rom) == 4096, len(rom)

def screen_code(c):
    o = ord(c)
    if 'a' <= c <= 'z':
        return o - 96          # mixed-case bank: a-z = 1..26
    if c == '@':
        return 0
    if c == '[':
        return 27
    if c == ']':
        return 29
    return o                   # space..? = 0x20..0x3F, A-Z = 65..90

chars = [chr(o) for o in range(0x20, 0x7B) if chr(o) not in "\\^_`"]
print("/// C64 character ROM, mixed-case set (second 2 KB bank: a–z = screen")
print("/// codes 1–26, A–Z = 65–90), 8×8, MSB = leftmost pixel. Bytes from")
print("/// `cbmeeks/VgaPico` `fonts/petscii/chargen`.")
print(f"const C64_GLYPHS: [(char, [u8; 8]); {len(chars)}] = [")
for c in chars:
    k = screen_code(c)
    row = ", ".join(f"0x{b:02X}" for b in rom[0x800 + k * 8: 0x800 + k * 8 + 8])
    lit = c.replace("'", "\\'")
    print(f"    ('{lit}', [{row}]),")
print("];")
```

Replace the existing `C64_GLYPHS` doc comment + const in `glyphs.rs` with the script's output (87 entries).

- [ ] **Step 4: Add the helpers**

```rust
/// Whether the C64 caption font has `ch`.
pub fn has_c64(ch: char) -> bool {
    C64_GLYPHS.iter().any(|(g, _)| *g == ch)
}

/// Width in pixels of `text` set in the C64 caption font.
pub fn text_width(text: &str) -> i64 {
    text.chars().count() as i64 * 8
}

/// Set `text` in the C64 caption font with its top-left at `at`, in the
/// letter style (one extrusion step).
pub fn stamp_text(c: &mut Canvas, text: &str, at: (i64, i64), face: &mut Vec<(i64, i64, Color)>) {
    for (i, ch) in text.chars().enumerate() {
        stamp(c, &c64(ch), (at.0 + i as i64 * 8, at.1), 1, 1, face);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --example hills`
Expected: all pass (3 new + smoke test).

- [ ] **Step 6: Commit**

```bash
git add examples/hills/glyphs.rs
git commit -m "feat: full C64 caption font and stamp_text for hills"
```

---

### Task 5: Figlet loader

**Files:**
- Create: `examples/hills/figlet.rs`
- Modify: `examples/hills/main.rs` (`mod figlet;`)

**Interfaces:**
- Produces: `pub struct FigletFont { pub height: usize, … }`, `FigletFont::parse(src: &str) -> Self`, `render(&self, text: &str) -> Vec<String>`, `width(&self, text: &str) -> usize`; `pub fn colossal() -> FigletFont`; `pub fn basic() -> FigletFont`; `pub fn fit(text: &str, max_w: usize) -> Vec<String>` (Colossal if it fits, else Basic).

- [ ] **Step 1: Write the failing tests** (in the new `figlet.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_load_with_expected_heights() {
        assert_eq!(colossal().height, 11);
        assert_eq!(basic().height, 8);
    }

    #[test]
    fn render_rows_and_width_add_up() {
        let f = colossal();
        let rows = f.render("AB");
        assert_eq!(rows.len(), 11);
        assert_eq!(f.width("AB"), f.width("A") + f.width("B"));
        assert!(rows.iter().all(|r| r.chars().count() == f.width("AB")));
    }

    #[test]
    fn fit_falls_back_to_basic() {
        let wide = colossal().width("VECTORFIELD");
        assert_eq!(fit("VECTORFIELD", wide).len(), 11);
        assert_eq!(fit("VECTORFIELD", wide - 1).len(), 8);
    }
}
```

Add `mod figlet;` to `main.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills figlet`
Expected: FAIL to compile — `colossal`, `basic`, `fit` not found.

- [ ] **Step 3: Implement** (above the tests)

Copy `struct FigletFont` and its `impl` (`parse`, `glyph`, `glyph_width`, `render`, `width`) verbatim from `examples/greetings.rs` lines 41–115, including the comment block above it; make the struct, `height`, `parse`, `render` and `width` `pub`. Then add:

```rust
const COLOSSAL_FLF: &str = include_str!("../fonts/colossal.flf");
const BASIC_FLF: &str = include_str!("../fonts/basic.flf");

pub fn colossal() -> FigletFont {
    FigletFont::parse(COLOSSAL_FLF)
}

pub fn basic() -> FigletFont {
    FigletFont::parse(BASIC_FLF)
}

/// `text` in Colossal when it fits in `max_w` columns, else in Basic.
pub fn fit(text: &str, max_w: usize) -> Vec<String> {
    let big = colossal();
    if big.width(text) <= max_w {
        big.render(text)
    } else {
        basic().render(text)
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --example hills figlet`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add examples/hills/figlet.rs examples/hills/main.rs
git commit -m "feat: figlet loader for hills panels"
```

---

### Task 6: Stage — storyboard, `locate`, seeking, clock

**Files:**
- Create: `examples/hills/stage.rs`
- Modify: `examples/hills/main.rs` (`mod stage;`)

**Interfaces:**
- Consumes: `glyphs::has_c64` (Task 4) in a test.
- Produces:

```rust
pub enum PanelKind { Plasma, Rainbow, Glow, Pulse, Neon, Radar, Karaoke, Glitch, Flap, Scroll, Spread,
                     Marquee, VSineWave, Dycp, Fld, Wind, VectorField, HeightMap, Screen, Multiply, Transition }
pub enum StepKind { Intro, LogoExit, Heading { title: &'static str },
                    Panel { kind: PanelKind, name: &'static str, subtitle: &'static str }, Finale }
pub struct Step { pub kind: StepKind, pub block: usize, pub secs: f64 }
pub const HEADING_SECS: f64 = 1.2;
pub fn storyboard() -> Vec<Step>
pub fn total(steps: &[Step]) -> f64
pub fn start_of(steps: &[Step], i: usize) -> f64
pub fn locate(steps: &[Step], t: f64) -> (usize, f64)
pub fn loop_target(steps: &[Step]) -> usize
pub enum Seek { Next, Prev, NextBlock, PrevBlock, Block(usize) }
pub fn seek_target(steps: &[Step], current: usize, seek: Seek) -> usize
pub struct Clock; Clock::new(); Clock::at(t: f64) -> Clock  // reads t at wall time 0
Clock::time(&mut self, steps: &[Step], now: f64, seek: Option<Seek>) -> f64
```

All enums/structs derive `Debug, Clone, Copy, PartialEq` (`Clock` derives `Debug` only).

- [ ] **Step 1: Write the failing tests** (in the new `stage.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn first_of_block(steps: &[Step], block: usize) -> usize {
        steps.iter().position(|s| s.block == block).unwrap()
    }

    #[test]
    fn storyboard_shape() {
        let s = storyboard();
        assert_eq!(s[0].kind, StepKind::Intro);
        assert_eq!(s[1].kind, StepKind::LogoExit);
        assert_eq!(s.last().unwrap().kind, StepKind::Finale);
        let panels = |b| s.iter().filter(|st| st.block == b && matches!(st.kind, StepKind::Panel { .. })).count();
        assert_eq!((panels(2), panels(3), panels(4), panels(5)), (7, 9, 2, 3));
        assert!(s.windows(2).all(|w| w[0].block <= w[1].block));
    }

    #[test]
    fn captions_use_only_c64_glyphs() {
        for st in storyboard() {
            let text = match st.kind {
                StepKind::Heading { title } => title,
                StepKind::Panel { subtitle, .. } => subtitle,
                _ => continue,
            };
            for ch in text.chars() {
                assert!(crate::glyphs::has_c64(ch), "{text:?} needs {ch:?}");
            }
        }
    }

    #[test]
    fn locate_boundaries() {
        let s = storyboard();
        assert_eq!(locate(&s, 0.0), (0, 0.0));
        assert_eq!(locate(&s, start_of(&s, 1)), (1, 0.0));
        let (i, local) = locate(&s, start_of(&s, 1) - 0.001);
        assert_eq!(i, 0);
        assert!((local - (s[0].secs - 0.001)).abs() < 1e-9);
        assert_eq!(locate(&s, total(&s) + 5.0).0, s.len() - 1);
    }

    #[test]
    fn next_and_prev() {
        let s = storyboard();
        assert_eq!(seek_target(&s, 3, Seek::Next), 4);
        assert_eq!(seek_target(&s, 3, Seek::Prev), 2);
    }

    #[test]
    fn prev_at_start_stays() {
        assert_eq!(seek_target(&storyboard(), 0, Seek::Prev), 0);
    }

    #[test]
    fn next_on_finale_loops() {
        let s = storyboard();
        assert_eq!(seek_target(&s, s.len() - 1, Seek::Next), loop_target(&s));
        assert_eq!(s[loop_target(&s)].kind, StepKind::LogoExit);
    }

    #[test]
    fn block_seeks() {
        let s = storyboard();
        let b3 = first_of_block(&s, 3);
        assert_eq!(seek_target(&s, 0, Seek::NextBlock), first_of_block(&s, 2));
        assert_eq!(seek_target(&s, b3 + 2, Seek::PrevBlock), first_of_block(&s, 2));
        assert_eq!(seek_target(&s, 0, Seek::Block(4)), first_of_block(&s, 4));
        assert_eq!(seek_target(&s, s.len() - 1, Seek::NextBlock), loop_target(&s));
    }

    #[test]
    fn block_jump_clamps() {
        let s = storyboard();
        assert_eq!(seek_target(&s, 5, Seek::Block(0)), 0);
        assert_eq!(seek_target(&s, 5, Seek::Block(9)), first_of_block(&s, 6));
    }

    #[test]
    fn clock_loops_after_finale() {
        let s = storyboard();
        let mut clock = Clock::new();
        let t = clock.time(&s, total(&s) + 0.5, None);
        assert!((t - (start_of(&s, loop_target(&s)) + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn clock_at_reads_t_at_zero() {
        let s = storyboard();
        assert!((Clock::at(20.0).time(&s, 0.0, None) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn clock_applies_seek_once() {
        let s = storyboard();
        let mut clock = Clock::new();
        let b3 = start_of(&s, first_of_block(&s, 3));
        assert!((clock.time(&s, 10.0, Some(Seek::Block(3))) - b3).abs() < 1e-9);
        // One second later, no new request: time runs on from the new origin.
        assert!((clock.time(&s, 11.0, None) - (b3 + 1.0)).abs() < 1e-9);
    }
}
```

Add `mod stage;` to `main.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills stage`
Expected: FAIL to compile — `storyboard`, `Step`, … not found.

- [ ] **Step 3: Implement** (above the tests)

```rust
//! The show's running order and the clock that walks it.

pub const INTRO_SECS: f64 = 7.0;
pub const LOGO_EXIT_SECS: f64 = 1.5;
pub const HEADING_SECS: f64 = 1.2;
pub const PANEL_SECS: f64 = 5.0;
pub const FIELD_SECS: f64 = 6.0;
pub const FINALE_SECS: f64 = 6.0;
pub const LAST_BLOCK: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelKind {
    Plasma, Rainbow, Glow, Pulse, Neon, Radar, Karaoke,
    Glitch, Flap, Scroll, Spread, Marquee, VSineWave, Dycp, Fld, Wind,
    VectorField, HeightMap,
    Screen, Multiply, Transition,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepKind {
    Intro,
    LogoExit,
    Heading { title: &'static str },
    Panel { kind: PanelKind, name: &'static str, subtitle: &'static str },
    Finale,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub kind: StepKind,
    /// 1 = intro, 2 = color, 3 = shape & layout, 4 = fields, 5 = mixing, 6 = finale.
    pub block: usize,
    pub secs: f64,
}

/// The whole show, in order.
pub fn storyboard() -> Vec<Step> {
    use PanelKind::*;
    let step = |kind, block, secs| Step { kind, block, secs };
    let heading = |title, block| step(StepKind::Heading { title }, block, HEADING_SECS);
    let panel = |kind, name, subtitle, block, secs| step(StepKind::Panel { kind, name, subtitle }, block, secs);

    let mut s = vec![step(StepKind::Intro, 1, INTRO_SECS), step(StepKind::LogoExit, 1, LOGO_EXIT_SECS)];

    s.push(heading("COLOR EFFECTS", 2));
    for (kind, name, sub) in [
        (Plasma, "PLASMA", "smooth as butter"),
        (Rainbow, "RAINBOW", "every hue, all the time"),
        (Glow, "GLOW", "a spotlight with a gradient"),
        (Pulse, "PULSE", "heartbeat from the centre"),
        (Neon, "NEON", "buzzing like a bar sign"),
        (Radar, "RADAR", "sweep, sweep, ping"),
        (Karaoke, "KARAOKE", "sing along, one letter at a time"),
    ] {
        s.push(panel(kind, name, sub, 2, PANEL_SECS));
    }

    s.push(heading("SHAPE & LAYOUT", 3));
    for (kind, name, sub) in [
        (Glitch, "GLITCH", "signal lost"),
        (Flap, "FLAP", "departures, now boarding"),
        (Scroll, "SCROLL", "sliding into place"),
        (Spread, "SPREAD", "fanning out from one line"),
        (Marquee, "MARQUEE", "round and round"),
        (VSineWave, "VSINEWAVE", "riding the wave"),
        (Dycp, "DYCP", "every letter its own height"),
        (Fld, "FLD", "rows floating free"),
        (Wind, "WIND", "blown away"),
    ] {
        s.push(panel(kind, name, sub, 3, PANEL_SECS));
    }

    // Fields fill the panel, so their name rides in the subtitle.
    s.push(heading("FIELDS", 4));
    s.push(panel(VectorField, "VECTORFIELD", "VectorField: follow the flow", 4, FIELD_SECS));
    s.push(panel(HeightMap, "HEIGHTMAP", "HeightMap: hills you can read", 4, FIELD_SECS));

    s.push(heading("MIXING", 5));
    for (kind, name, sub) in [
        (Screen, "SCREEN", "light on light"),
        (Multiply, "MULTIPLY", "shadows with colour"),
        (Transition, "TRANSITION", "from one to the other"),
    ] {
        s.push(panel(kind, name, sub, 5, PANEL_SECS));
    }

    s.push(step(StepKind::Finale, LAST_BLOCK, FINALE_SECS));
    s
}

pub fn total(steps: &[Step]) -> f64 {
    steps.iter().map(|s| s.secs).sum()
}

pub fn start_of(steps: &[Step], i: usize) -> f64 {
    steps[..i].iter().map(|s| s.secs).sum()
}

/// Which step is playing at storyboard time `t`, and how far into it.
/// Before 0 → step 0; at or past the end → the last step (clamped).
pub fn locate(steps: &[Step], t: f64) -> (usize, f64) {
    let mut start = 0.0;
    for (i, s) in steps.iter().enumerate() {
        if t < start + s.secs || i == steps.len() - 1 {
            return (i, (t - start).max(0.0).min(s.secs));
        }
        start += s.secs;
    }
    unreachable!("storyboard is never empty")
}

/// Where the show continues after the finale: the logo exit.
pub fn loop_target(steps: &[Step]) -> usize {
    steps.iter().position(|s| s.kind == StepKind::LogoExit).expect("storyboard has a logo exit")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seek {
    Next,
    Prev,
    NextBlock,
    PrevBlock,
    Block(usize),
}

fn first_of(steps: &[Step], block: usize) -> Option<usize> {
    steps.iter().position(|s| s.block == block)
}

/// The step a seek request lands on, from step `current`.
pub fn seek_target(steps: &[Step], current: usize, seek: Seek) -> usize {
    let block = steps[current].block;
    match seek {
        Seek::Next if current + 1 < steps.len() => current + 1,
        Seek::Next => loop_target(steps),
        Seek::Prev => current.saturating_sub(1),
        Seek::NextBlock => first_of(steps, block + 1).unwrap_or_else(|| loop_target(steps)),
        Seek::PrevBlock => first_of(steps, block.saturating_sub(1).max(1)).unwrap_or(0),
        Seek::Block(n) => first_of(steps, n.clamp(1, LAST_BLOCK)).unwrap_or(0),
    }
}

/// Maps wall time to storyboard time. Seeking moves the origin; running
/// past the finale moves it back to the loop target.
#[derive(Debug)]
pub struct Clock {
    origin: f64,
}

impl Clock {
    pub fn new() -> Self {
        Self { origin: 0.0 }
    }

    /// A clock that reads storyboard time `t` at wall time 0.
    pub fn at(t: f64) -> Self {
        Self { origin: -t }
    }

    /// Storyboard time at wall time `now` (seconds), after applying `seek`.
    pub fn time(&mut self, steps: &[Step], now: f64, seek: Option<Seek>) -> f64 {
        if let Some(seek) = seek {
            let (current, _) = locate(steps, now - self.origin);
            self.origin = now - start_of(steps, seek_target(steps, current, seek));
        }
        let (end, back) = (total(steps), start_of(steps, loop_target(steps)));
        while now - self.origin >= end {
            self.origin += end - back;
        }
        now - self.origin
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --example hills stage`
Expected: 11 passed.

- [ ] **Step 5: Commit**

```bash
git add examples/hills/stage.rs examples/hills/main.rs
git commit -m "feat: hills storyboard, seeking and looping clock"
```

---

### Task 7: Wordmark enter / exit phases

**Files:**
- Modify: `examples/hills/wordmark.rs`, `examples/hills/main.rs` (`build_layers` call)

**Interfaces:**
- Consumes: `Wordmark` (Task 3).
- Produces: `pub enum LogoPhase { Hidden, Enter(f64), Exit(f64) }` (local seconds; `Debug, Clone, Copy, PartialEq`); `Wordmark::paint(&self, c: &mut Canvas, x0: i64, y0: i64, phase: LogoPhase)`; `pub const EXIT_STAGGER: f64 = 0.08; pub const EXIT_SECS: f64 = 0.6;`; `fn drop_offset(i: usize, t: f64) -> f64`; `fn exit_offset(i: usize, t: f64) -> f64`.

- [ ] **Step 1: Write the failing tests** (append to `wordmark.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_starts_off_screen_and_lands() {
        assert_eq!(drop_offset(0, 0.0), -FALL);
        assert!(drop_offset(10, DROP_START + 10.0 * DROP_STAGGER + DROP_SECS).abs() < 1e-6);
    }

    #[test]
    fn exit_starts_in_place_and_ends_off_screen() {
        assert_eq!(exit_offset(0, 0.0), 0.0);
        for i in 0..WORD.len() {
            assert_eq!(exit_offset(i, super::super::stage::LOGO_EXIT_SECS), -FALL);
        }
    }

    #[test]
    fn exit_is_staggered() {
        let t = 0.3;
        assert!(exit_offset(0, t) < exit_offset(5, t));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills wordmark`
Expected: FAIL to compile — `drop_offset`, `exit_offset` not found.

- [ ] **Step 3: Implement**

Add near the other constants:

```rust
// Exit: letter i rises off the top starting at i × EXIT_STAGGER seconds,
// taking EXIT_SECS (ease-in) — 11 letters finish within 1.5 s.
pub const EXIT_STAGGER: f64 = 0.08;
pub const EXIT_SECS: f64 = 0.6;

/// What the wordmark is doing, with local seconds into that phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogoPhase {
    Hidden,
    Enter(f64),
    Exit(f64),
}

/// Letter `i`'s drop-in offset `t` seconds into the entrance (−FALL → 0).
fn drop_offset(i: usize, t: f64) -> f64 {
    let falling = t - (DROP_START + i as f64 * DROP_STAGGER);
    -FALL * (1.0 - Easing::BounceOut.apply(falling / DROP_SECS))
}

/// Letter `i`'s lift `t` seconds into the exit (0 → −FALL).
fn exit_offset(i: usize, t: f64) -> f64 {
    -FALL * Easing::EaseIn.apply((t - i as f64 * EXIT_STAGGER) / EXIT_SECS)
}
```

Replace `bob(&self, i, frame)` with a version that takes wall time for the wave and the phase for the drop/exit:

```rust
    /// Letter `i`'s vertical offset: phase movement plus the DYCP wave,
    /// which runs on wall time so it never jumps between phases.
    fn bob(&self, i: usize, wall: f64, phase: LogoPhase) -> i64 {
        let (shift, swell) = match phase {
            LogoPhase::Hidden => return 0,
            LogoPhase::Enter(t) => {
                let landed = t - (DROP_START + i as f64 * DROP_STAGGER) - DROP_SECS;
                (drop_offset(i, t), Easing::EaseInOut.apply(landed / DYCP_RAMP))
            }
            LogoPhase::Exit(t) => (exit_offset(i, t), 1.0),
        };
        let i = i as f64;
        let main = (TAU * wall / DYCP_PERIOD - i * DYCP_SPREAD).sin();
        let counter = (TAU * wall / (DYCP_PERIOD * 1.9) + i * DYCP_SPREAD * 0.6).sin();
        let breathe = 0.8 + 0.35 * (TAU * wall / DYCP_BREATHE).sin();
        let s = (0.7 * main + 0.45 * counter) * breathe;
        (shift + swell * DYCP_AMP * self.swing_scale() * s).round() as i64
    }
```

`paint` gains the phase and skips `Hidden`:

```rust
    pub fn paint(&self, c: &mut Canvas, x0: i64, y0: i64, phase: LogoPhase) {
        if phase == LogoPhase::Hidden {
            return;
        }
        let wall = c.frame as f64 / FPS;
        let s = self.scale as i64;
        let mut face = Vec::new();
        for (i, glyph) in self.glyphs.iter().enumerate() {
            let at = (x0 + (i * ADVANCE) as i64 * s, y0 + self.bob(i, wall, phase));
            stamp(c, glyph, at, s, 2, &mut face);
        }
    }
```

In `main.rs` `build_layers`, keep today's behaviour until Task 9 by passing the intro phase on wall time:

```rust
        .layer(Layer::paint(move |c| {
            let t = c.frame as f64 / chromakopia::animate::framebuffer::FPS;
            word.paint(c, x0, y0, wordmark::LogoPhase::Enter(t))
        }))
```

- [ ] **Step 4: Run tests**

Run: `cargo test --example hills`
Expected: all pass (3 new).

- [ ] **Step 5: Commit**

```bash
git add examples/hills/wordmark.rs examples/hills/main.rs
git commit -m "feat: wordmark enter/exit phases driven by local time"
```

---

### Task 8: Panel — geometry, motion, frame, effects, blit

**Files:**
- Create: `examples/hills/panel.rs`
- Modify: `examples/hills/main.rs` (`mod panel;`)

**Interfaces:**
- Consumes: `glyphs::{LETTER_OUTLINE, LETTER_EXTRUDE}`; `stage::PanelKind`; `main::ROWS`.
- Produces:

```rust
pub const PANEL_FILL: Color;   // #2A1638
pub const PANEL_EDGE: Color;   // #120A18
pub const ENTER_SECS: f64 = 0.7;
pub const EXIT_SECS: f64 = 0.5;
pub struct Geometry { pub x: i64, pub y: i64, pub w: usize, pub h: usize }   // cells; y = resting row
pub fn geometry(screen_w: usize, block: usize) -> Geometry
pub fn panel_y(geo: &Geometry, local: f64, secs: f64) -> i64                  // whole cells
pub fn at_rest(local: f64, secs: f64) -> bool
pub fn paint_frame(c: &mut Canvas, geo: &Geometry, y: i64)
pub fn effect_for(kind: PanelKind, text: &str) -> Box<dyn Effect>
pub fn blit(buf: &mut FrameBuffer, geo: &Geometry, y: i64, effect: &dyn Effect, frame: usize)
```

- [ ] **Step 1: Write the failing tests** (in the new `panel.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_is_centred() {
        let g = geometry(200, 2);
        assert_eq!((g.w, g.h, g.y), (104, 13, 9));
        assert_eq!(g.x, (200 - 104) / 2);
        assert_eq!(geometry(200, 4).h, 15);
    }

    #[test]
    fn geometry_clamps_to_narrow_screen() {
        let g = geometry(60, 2);
        assert_eq!(g.w, 50);
        assert_eq!(g.x, 5);
        assert_eq!(geometry(10, 2).w, 20); // never below the minimum
    }

    #[test]
    fn panel_bounces_in_rests_and_falls_out() {
        let g = geometry(200, 2);
        assert!(panel_y(&g, 0.0, 5.0) < 0);
        assert_eq!(panel_y(&g, ENTER_SECS, 5.0), g.y);
        assert_eq!(panel_y(&g, 2.5, 5.0), g.y);
        assert!(panel_y(&g, 5.0, 5.0) >= super::super::ROWS as i64);
        assert!(at_rest(2.5, 5.0) && !at_rest(0.1, 5.0) && !at_rest(4.9, 5.0));
    }

    #[test]
    fn blit_paints_glyphs_on_panel_fill() {
        let g = Geometry { x: 2, y: 1, w: 6, h: 3 };
        let mut buf = FrameBuffer::new(10, 5);
        blit(&mut buf, &g, 1, &text("X", Color::new(255, 0, 0)), 0);
        // 1×1 content centred in a 6×3 interior: column 2 + 5/2 = 4, row 1 + 2/2 = 2.
        let cell = buf.get(4, 2);
        assert_eq!((cell.ch, cell.bg), ('X', Some(PANEL_FILL)));
    }

    #[test]
    fn blit_clips_to_small_buffer() {
        let g = geometry(200, 2);
        let mut buf = FrameBuffer::new(30, 4); // far smaller than the panel
        blit(&mut buf, &g, g.y, &text("WIDE TEXT", Color::new(255, 255, 255)), 0);
    }

    #[test]
    fn every_kind_builds_and_renders() {
        use super::super::stage::{storyboard, StepKind};
        for st in storyboard() {
            if let StepKind::Panel { kind, name, .. } = st.kind {
                let fx = effect_for(kind, name);
                let mut buf = FrameBuffer::new(60, 12);
                fx.render(&mut buf, 30);
            }
        }
    }
}
```

Add `mod panel;` to `main.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills panel`
Expected: FAIL to compile — `geometry`, `panel_y`, … not found.

- [ ] **Step 3: Implement** (above the tests)

```rust
//! The styled panel each effect is shown on.

use chromakopia::animate::Cell;
use chromakopia::prelude::*;

use super::glyphs::{LETTER_EXTRUDE, LETTER_OUTLINE};
use super::stage::PanelKind;
use super::ROWS;

/// Panel fill — the letter-outline aubergine.
pub const PANEL_FILL: Color = LETTER_OUTLINE;
/// One-pixel edge around the fill, a shade darker.
pub const PANEL_EDGE: Color = Color { r: 0x12, g: 0x0A, b: 0x18 };

pub const ENTER_SECS: f64 = 0.7;
pub const EXIT_SECS: f64 = 0.5;

const MAX_W: usize = 104;
const MIN_W: usize = 20;
const MARGIN: usize = 10;
/// Resting top row: leaves room for the heading above.
const REST_Y: i64 = 9;

/// Panel interior in cells. `y` is the resting row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub x: i64,
    pub y: i64,
    pub w: usize,
    pub h: usize,
}

pub fn geometry(screen_w: usize, block: usize) -> Geometry {
    let w = screen_w.saturating_sub(MARGIN).clamp(MIN_W, MAX_W);
    let h = if block == 4 { 15 } else { 13 };
    Geometry { x: (screen_w as i64 - w as i64) / 2, y: REST_Y, w, h }
}

/// Panel top row `local` seconds into a step of `secs`: bounces in from
/// above, rests, falls out below. Always a whole cell.
pub fn panel_y(geo: &Geometry, local: f64, secs: f64) -> i64 {
    let above = -(geo.h as f64) - 3.0;
    let below = ROWS as f64 + 2.0;
    let rest = geo.y as f64;
    let y = if local < ENTER_SECS {
        above + (rest - above) * Easing::BounceOut.apply(local / ENTER_SECS)
    } else if local > secs - EXIT_SECS {
        rest + (below - rest) * Easing::EaseIn.apply((local - (secs - EXIT_SECS)) / EXIT_SECS)
    } else {
        rest
    };
    y.round() as i64
}

/// Between the bounce-in and the fall-out.
pub fn at_rest(local: f64, secs: f64) -> bool {
    local >= ENTER_SECS && local <= secs - EXIT_SECS
}

/// Fill `[x0, x1) × [y0, y1)` in canvas pixels.
fn fill(c: &mut Canvas, (x0, y0, x1, y1): (i64, i64, i64, i64), color: Color) {
    for y in y0..y1 {
        for x in x0..x1 {
            c.set(x, y, color);
        }
    }
}

/// Two-step extrusion down-right, one-pixel edge, then the fill — the
/// letters' look, as a box. `y` is the panel's current top row (cells).
pub fn paint_frame(c: &mut Canvas, geo: &Geometry, y: i64) {
    let inner = (geo.x, y * 2, geo.x + geo.w as i64, (y + geo.h as i64) * 2);
    let outer = (inner.0 - 1, inner.1 - 1, inner.2 + 1, inner.3 + 1);
    for k in [2, 1] {
        fill(c, (outer.0 + k, outer.1 + k, outer.2 + k, outer.3 + k), LETTER_EXTRUDE);
    }
    fill(c, outer, PANEL_EDGE);
    fill(c, inner, PANEL_FILL);
}

/// The effect a panel shows, driven over `text` (the figlet-rendered name).
pub fn effect_for(kind: PanelKind, text: &str) -> Box<dyn Effect> {
    use PanelKind as K;
    match kind {
        K::Plasma => Box::new(Plasma::on(text)),
        K::Rainbow => Box::new(Rainbow::on(text)),
        K::Glow => Box::new(Glow::on(text)),
        K::Pulse => Box::new(Pulse::on(text)),
        K::Neon => Box::new(Neon::on(text)),
        K::Radar => Box::new(Radar::on(text)),
        K::Karaoke => Box::new(Karaoke::on(text)),
        K::Glitch => Box::new(Glitch::new(text)),
        K::Flap => Box::new(Flap::new(text)),
        K::Scroll => Box::new(Scroll::new(text)),
        K::Spread => Box::new(Spread::new(text)),
        K::Marquee => Box::new(Marquee::wrap(Rainbow::on(text)).speed(20.0)),
        K::VSineWave => Box::new(VSineWave::wrap(Plasma::on(text))),
        K::Dycp => Box::new(Dycp::new(text)),
        K::Fld => Box::new(Fld::new(Plasma::on(text))),
        K::Wind => Box::new(Wind::wrap(Plasma::on(text))),
        K::VectorField => Box::new(VectorField::new()),
        K::HeightMap => Box::new(HeightMap::new()),
        K::Screen => Box::new(Blend::new(Plasma::on(text), Radar::on(text), BlendMode::Screen)),
        K::Multiply => Box::new(Blend::new(Rainbow::on(text), Glow::on(text), BlendMode::Multiply)),
        K::Transition => Box::new(Transition::new(Plasma::on(text), Rainbow::on(text), 2.5, Easing::EaseInOut)),
    }
}

/// Render `effect` at `frame` and lay it over the panel interior at row
/// `y`, centred. Sized effects keep their size; unsized ones (fields) fill
/// the interior. Glyph cells get the panel fill as background; everything
/// is clipped to `buf`.
pub fn blit(buf: &mut FrameBuffer, geo: &Geometry, y: i64, effect: &dyn Effect, frame: usize) {
    let (ew, eh) = effect.size();
    let (w, h) = if ew == 0 || eh == 0 { (geo.w, geo.h) } else { (ew.min(geo.w), eh.min(geo.h)) };
    let mut sub = FrameBuffer::new(w, h);
    effect.render(&mut sub, frame);
    let ox = geo.x + (geo.w - w) as i64 / 2;
    let oy = y + (geo.h - h) as i64 / 2;
    for sy in 0..h {
        for sx in 0..w {
            let cell = sub.get(sx, sy);
            if cell.ch == ' ' {
                continue;
            }
            let (tx, ty) = (ox + sx as i64, oy + sy as i64);
            if tx >= 0 && ty >= 0 && (tx as usize) < buf.width && (ty as usize) < buf.height {
                buf.set(tx as usize, ty as usize, Cell { bg: Some(PANEL_FILL), ..cell });
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --example hills panel`
Expected: 6 passed. If `every_kind_builds_and_renders` fails to compile for one constructor (e.g. a builder returning a different type), check that effect's `pub fn` in `src/animate/effects.rs` and adapt only that arm.

- [ ] **Step 5: Commit**

```bash
git add examples/hills/panel.rs examples/hills/main.rs
git commit -m "feat: styled effect panel with bounce motion and blit"
```

---

### Task 9: Stage effect, captions, keys — wire it all up

**Files:**
- Modify: `examples/hills/main.rs`

**Interfaces:**
- Consumes: everything above — `landscape::landscape`, `wordmark::{Wordmark, LogoPhase}`, `scroller::{Scroller, LICENSE_TEXT}`, `glyphs::{stamp_text, text_width}`, `figlet::fit`, `stage::*`, `panel::*`, `Key`, `Scene::on_key`.
- Produces: `struct View`, `fn view_for(steps: &[Step], i: usize, local: f64) -> View`, `struct Stage` implementing `Effect`.

- [ ] **Step 1: Write the failing tests** (replace `mod tests` in `main.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use stage::{start_of, storyboard, StepKind};

    #[test]
    fn intro_and_exit_drive_the_logo() {
        let s = storyboard();
        assert_eq!(view_for(&s, 0, 2.0).logo, wordmark::LogoPhase::Enter(2.0));
        assert_eq!(view_for(&s, 1, 0.5).logo, wordmark::LogoPhase::Exit(0.5));
        assert!(view_for(&s, 0, 2.0).heading.is_none());
    }

    #[test]
    fn heading_drops_then_stays_for_its_block() {
        let s = storyboard();
        let h = s.iter().position(|st| matches!(st.kind, StepKind::Heading { .. })).unwrap();
        let (title, p) = view_for(&s, h, stage::HEADING_SECS / 2.0).heading.unwrap();
        assert_eq!((title, p), ("COLOR EFFECTS", 0.5));
        assert_eq!(view_for(&s, h + 3, 1.0).heading, Some(("COLOR EFFECTS", 1.0)));
        assert_eq!(view_for(&s, h + 3, 1.0).logo, wordmark::LogoPhase::Hidden);
    }

    #[test]
    fn panel_steps_expose_a_panel() {
        let s = storyboard();
        let p = s.iter().position(|st| matches!(st.kind, StepKind::Panel { .. })).unwrap();
        let v = view_for(&s, p, 1.0);
        assert_eq!(v.panel.map(|pv| (pv.step, pv.local)), Some((p, 1.0)));
    }

    #[test]
    fn finale_shows_logo_only() {
        let s = storyboard();
        let v = view_for(&s, s.len() - 1, 1.0);
        assert_eq!((v.logo, v.heading, v.panel), (wordmark::LogoPhase::Enter(1.0), None, None));
    }

    #[test]
    fn stage_renders_every_block() {
        let s = storyboard();
        let stage = Stage::new(120);
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            stage.seek_to(start_of(&s, i) + 2.0);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            assert!(b.to_ansi_string().contains('▀'), "block {block}");
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --example hills`
Expected: FAIL to compile — `view_for`, `Stage` not found.

- [ ] **Step 3: Implement `View` and `view_for`**

```rust
use std::sync::{Arc, Mutex};

use chromakopia::animate::framebuffer::FPS;
use stage::{locate, storyboard, Clock, Seek, Step, StepKind};
use wordmark::LogoPhase;

/// A panel step in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelView {
    step: usize,
    local: f64,
}

/// Everything the paint layers need to know about this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    logo: LogoPhase,
    /// Heading text and its drop-in progress (0..=1).
    heading: Option<(&'static str, f64)>,
    panel: Option<PanelView>,
}

/// What is on screen `local` seconds into step `i`.
fn view_for(steps: &[Step], i: usize, local: f64) -> View {
    let step = steps[i];
    let logo = match step.kind {
        StepKind::Intro | StepKind::Finale => LogoPhase::Enter(local),
        StepKind::LogoExit => LogoPhase::Exit(local),
        _ => LogoPhase::Hidden,
    };
    let heading = steps[..=i]
        .iter()
        .rposition(|s| s.block == step.block && matches!(s.kind, StepKind::Heading { .. }))
        .map(|h| {
            let StepKind::Heading { title } = steps[h].kind else { unreachable!() };
            let progress = if h == i { (local / stage::HEADING_SECS).min(1.0) } else { 1.0 };
            (title, progress)
        });
    let panel = matches!(step.kind, StepKind::Panel { .. }).then_some(PanelView { step: i, local });
    View { logo, heading, panel }
}
```

- [ ] **Step 4: Implement `Stage`**

```rust
/// Plays the storyboard: applies seeks, publishes the frame's `View` for
/// the paint layers, renders them, then blits the active panel effect.
struct Stage {
    steps: Vec<Step>,
    /// Per step: the panel's effect (None for non-panel steps).
    effects: Vec<Option<Box<dyn Effect>>>,
    screen_w: usize,
    layers: Layers,
    view: Arc<Mutex<View>>,
    clock: Mutex<Clock>,
    seek: Arc<Mutex<Option<Seek>>>,
}

impl Stage {
    fn new(screen_w: usize) -> Self {
        let steps = storyboard();
        let effects = steps
            .iter()
            .map(|st| match st.kind {
                StepKind::Panel { kind, name, .. } => {
                    let geo = panel::geometry(screen_w, st.block);
                    Some(panel::effect_for(kind, &figlet::fit(name, geo.w - 2).join("\n")))
                }
                _ => None,
            })
            .collect();
        let view = Arc::new(Mutex::new(view_for(&steps, 0, 0.0)));
        let layers = build_layers(screen_w, view.clone(), steps.clone());
        Self { steps, effects, screen_w, layers, view, clock: Mutex::new(Clock::new()), seek: Arc::new(Mutex::new(None)) }
    }

    /// Jump the clock so storyboard time is `t` at frame 0 (tests).
    #[cfg(test)]
    fn seek_to(&self, t: f64) {
        *self.clock.lock().unwrap() = Clock::at(t);
    }
}

impl Effect for Stage {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let now = frame as f64 / FPS;
        let seek = self.seek.lock().unwrap().take();
        let t = self.clock.lock().unwrap().time(&self.steps, now, seek);
        let (i, local) = locate(&self.steps, t);
        let view = view_for(&self.steps, i, local);
        *self.view.lock().unwrap() = view;

        self.layers.render(buf, frame);

        if let (Some(pv), Some(effect)) = (view.panel, &self.effects[i]) {
            let step = self.steps[pv.step];
            let geo = panel::geometry(self.screen_w, step.block);
            let y = panel::panel_y(&geo, pv.local, step.secs);
            panel::blit(buf, &geo, y, effect.as_ref(), (pv.local * FPS) as usize);
        }
    }

    fn size(&self) -> (usize, usize) {
        (0, ROWS)
    }
}
```

- [ ] **Step 5: Paint layers read the `View`**

Replace `build_layers` with:

```rust
fn build_layers(screen_w: usize, view: Arc<Mutex<View>>, steps: Vec<Step>) -> Layers {
    let (word, x0, y0) = wordmark::Wordmark::placed(screen_w);
    let scroller = scroller::Scroller::new(scroller::LICENSE_TEXT, (ROWS * 2) as i64 - 15);

    let logo_view = view.clone();
    let frame_view = view.clone();
    let caption_view = view;
    let frame_steps = steps.clone();
    let caption_steps = steps;

    landscape::landscape()
        .layer(Layer::paint(move |c| {
            let phase = logo_view.lock().unwrap().logo;
            word.paint(c, x0, y0, phase)
        }))
        // Panel box (fill, edge, extrusion).
        .layer(Layer::paint(move |c| {
            let Some(pv) = frame_view.lock().unwrap().panel else { return };
            let step = frame_steps[pv.step];
            let geo = panel::geometry(c.width, step.block);
            panel::paint_frame(c, &geo, panel::panel_y(&geo, pv.local, step.secs));
        }))
        // Heading above the panel, subtitle below it while it rests.
        .layer(Layer::paint(move |c| {
            let v = *caption_view.lock().unwrap();
            let mut face = Vec::new();
            let centre = |text: &str| (c.width as i64 - glyphs::text_width(text)) / 2;
            if let Some((title, progress)) = v.heading {
                let y = 4 - ((1.0 - Easing::BounceOut.apply(progress)) * 16.0).round() as i64;
                glyphs::stamp_text(c, title, (centre(title), y), &mut face);
            }
            if let Some(pv) = v.panel {
                let step = caption_steps[pv.step];
                if let StepKind::Panel { subtitle, .. } = step.kind {
                    if panel::at_rest(pv.local, step.secs) {
                        let geo = panel::geometry(c.width, step.block);
                        let y = (geo.y + geo.h as i64) * 2 + 4;
                        glyphs::stamp_text(c, subtitle, (centre(subtitle), y), &mut face);
                    }
                }
            }
        }))
        .layer(Layer::paint(move |c| scroller.paint(c)))
}
```

- [ ] **Step 6: `main` with keys**

```rust
#[tokio::main]
async fn main() {
    let stage = Stage::new(terminal_width());
    let seek = stage.seek.clone();
    Scene::new()
        .add(stage)
        .fullscreen()
        .quit_key('q')
        .on_key(move |key| {
            let request = match key {
                Key::Right | Key::Char('n') | Key::Char(' ') => Seek::Next,
                Key::Left | Key::Char('p') => Seek::Prev,
                Key::Down => Seek::NextBlock,
                Key::Up => Seek::PrevBlock,
                Key::Char(c @ '1'..='6') => Seek::Block(c as usize - '0' as usize),
                _ => return,
            };
            *seek.lock().unwrap() = Some(request);
        })
        .run(24.0 * 3600.0)
        .await;
}
```

Update the module doc comment at the top of `main.rs` to describe the showcase, the blocks and the keys (replace the old "Hills —" text; keep the `Run:` line, adding `→ ← ↑ ↓ 1–6 to skip`).

- [ ] **Step 7: Run tests and build**

Run: `cargo test --example hills && cargo build --release --example hills`
Expected: all tests pass; no warnings from `examples/hills`.

- [ ] **Step 8: Benchmark every block** (append to `mod tests`)

```rust
    /// `cargo test --release --example hills bench_blocks -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_blocks() {
        let s = storyboard();
        let stage = Stage::new(200);
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            stage.seek_to(start_of(&s, i) + 1.0);
            let start = std::time::Instant::now();
            for f in 0..120 {
                let mut b = FrameBuffer::new(200, ROWS);
                stage.render(&mut b, f);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / 120.0;
            println!("block {block}: {ms:.2} ms/frame");
            assert!(ms < 8.0, "block {block} too slow: {ms:.2} ms/frame");
        }
    }
```

Run: `cargo test --release --example hills bench_blocks -- --ignored --nocapture`
Expected: PASS, every block well under 8 ms/frame (half the 16.7 ms budget).

- [ ] **Step 9: Visual check per block** (append to `mod tests`)

```rust
    /// `cargo test --release --example hills dump_blocks -- --ignored`, then
    /// `cat target/hills-blocks.ansi` in a 120-column terminal.
    #[test]
    #[ignore]
    fn dump_blocks() {
        let s = storyboard();
        let stage = Stage::new(120);
        let mut out = String::new();
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            let target = if block == 1 { 0 } else { i + 1 }; // first panel of the block
            let target = target.min(s.len() - 1);
            stage.seek_to(start_of(&s, target) + 2.0);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== block {block} ===\n{}\n", b.to_ansi_string());
        }
        std::fs::write("target/hills-blocks.ansi", out).unwrap();
    }
```

Run it, `cat target/hills-blocks.ansi`, and check: the logo in block 1, a panel with heading and subtitle in blocks 2–5, the logo in block 6, the scroller everywhere, nothing torn.

- [ ] **Step 10: Manual run**

Run: `cargo run --release --example hills`. Check: the intro plays, the logo leaves upward, panels bounce in and out, `→ ← ↑ ↓ 1–6` skip, the show loops after the finale, `q` restores the terminal.

- [ ] **Step 11: Commit**

```bash
git add examples/hills
git commit -m "feat: hills showcase — storyboard of every effect on bouncing panels"
```
