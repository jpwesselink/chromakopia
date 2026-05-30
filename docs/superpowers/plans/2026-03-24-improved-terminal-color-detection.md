# Improved Terminal Color Detection

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make terminal background/foreground color detection robust across platforms and terminal emulators by adding multiple fallback strategies, probing both colors in a single TTY session, and exposing a `luma()` helper for light/dark theme detection.

**Architecture:** Extend the existing fallback chain in `terminal.rs` from 3 tiers (OSC → COLORFGBG → hardcoded) to 5 tiers (OSC → COLORFGBG → macOS system theme → TERM_PROGRAM heuristics → hardcoded). Merge the two separate `probe_osc_color` calls into a single TTY session via a new `probe_osc_colors` (plural) function. Add `Color::luma()` and `is_light_theme()` / `is_dark_theme()` convenience functions to the public API.

**Tech Stack:** Rust, libc (unix), std::process::Command (macOS `defaults read`), cfg-gated platform code

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/color.rs` | Modify | Add `luma()` method to `Color` |
| `src/terminal.rs` | Modify | Unified OSC probe, new fallback tiers, theme detection |
| `src/lib.rs` | Modify | Re-export new public functions |
| `src/animate/mod.rs` | No change | Already calls `probe_colors()` — benefits automatically |

---

### Task 1: Add `Color::luma()` method

**Files:**
- Modify: `src/color.rs` (inside `impl Color`, after `lerp_hsv` at line ~112)
- Test: `src/color.rs` (existing `#[cfg(test)]` block)

This is a pure function with no dependencies — do this first so later tasks can use it.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` block in `src/color.rs`:

```rust
#[test]
fn luma_black_is_zero() {
    assert!((Color::new(0, 0, 0).luma()).abs() < f64::EPSILON);
}

#[test]
fn luma_white_is_one() {
    assert!((Color::new(255, 255, 255).luma() - 1.0).abs() < 0.01);
}

#[test]
fn luma_green_brighter_than_blue() {
    // BT.709: green has much higher weight than blue
    let green = Color::new(0, 255, 0).luma();
    let blue = Color::new(0, 0, 255).luma();
    assert!(green > blue * 5.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test luma -- --nocapture`
Expected: FAIL — `luma` method does not exist

- [ ] **Step 3: Write minimal implementation**

Add to `impl Color` in `src/color.rs`, after the `lerp_hsv` method:

```rust
/// Perceived brightness (0.0 = black, 1.0 = white) using BT.709 coefficients.
///
/// Useful for determining if a color is "light" or "dark":
/// - luma > 0.5 → light
/// - luma < 0.5 → dark
pub fn luma(self) -> f64 {
    let r = self.r as f64 / 255.0;
    let g = self.g as f64 / 255.0;
    let b = self.b as f64 / 255.0;
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test luma -- --nocapture`
Expected: 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/color.rs
git commit -m "feat: add Color::luma() for perceptual brightness (BT.709)"
```

---

### Task 2: Merge dual OSC probes into a single TTY session

**Files:**
- Modify: `src/terminal.rs:13-117` (replace `probe_osc_color` with `probe_osc_colors`)
- Test: `src/terminal.rs` (existing `#[cfg(test)]` block)

Currently `bg_color()` and `fg_color()` each open `/dev/tty`, put it in raw mode, send a query, read a response, drain, and restore. This doubles the risk of leftover bytes bleeding between probes. Replace with a single function that probes both in one session.

- [ ] **Step 1: Delete the old `probe_osc_color` function**

Delete both the `#[cfg(unix)]` and `#[cfg(not(unix))]` variants of `probe_osc_color` (the function that takes a single `osc_code: u8`). These are being replaced by the unified `probe_osc_colors`.

- [ ] **Step 2: Implement `probe_osc_colors` (plural)**

Add the following to `src/terminal.rs`:

```rust
/// Probe both terminal foreground and background colors in a single `/dev/tty` session.
///
/// Sends OSC 10 (foreground) and OSC 11 (background) queries back-to-back
/// and parses both responses. Returns `(fg, bg)` where each is `None` if
/// that specific query failed.
#[cfg(unix)]
fn probe_osc_colors() -> (Option<Color>, Option<Color>) {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::unix::io::AsRawFd;
    use std::time::Duration;

    let Some(mut tty) = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
    else {
        return (None, None);
    };

    let fd = tty.as_raw_fd();

    if unsafe { libc::isatty(fd) } != 1 {
        return (None, None);
    }

    let old_termios = unsafe {
        let mut t = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut t) != 0 {
            return (None, None);
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
    let _guard = TermiosGuard { fd, original: old_termios };

    let mut raw = old_termios;
    unsafe {
        libc::cfmakeraw(&mut raw);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
            return (None, None);
        }
    }

    // Send both queries back-to-back in one write
    let query = "\x1B]10;?\x07\x1B]11;?\x07";
    if tty.write_all(query.as_bytes()).is_err() {
        return (None, None);
    }
    let _ = tty.flush();

    // Read all response bytes (expecting two OSC responses)
    let mut response = Vec::new();
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(200);
    let mut terminators_seen = 0u8;

    loop {
        if start.elapsed() > timeout || terminators_seen >= 2 {
            break;
        }
        let mut byte = [0u8; 1];
        match tty.read(&mut byte) {
            Ok(1) => {
                response.push(byte[0]);
                // BEL terminator
                if byte[0] == 0x07 && response.len() > 4 {
                    terminators_seen += 1;
                }
                // ST terminator (ESC \)
                if response.len() >= 2
                    && response[response.len() - 2] == 0x1B
                    && response[response.len() - 1] == b'\\'
                {
                    terminators_seen += 1;
                }
            }
            Ok(_) => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => break,
        }
    }

    // Drain leftover bytes
    {
        let mut byte = [0u8; 1];
        loop {
            match std::io::Read::read(&mut tty, &mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    }

    // _guard drops here, restoring old_termios

    // Split response into individual OSC replies and parse each
    let fg = parse_osc_response_for_code(&response, 10);
    let bg = parse_osc_response_for_code(&response, 11);
    (fg, bg)
}

#[cfg(not(unix))]
fn probe_osc_colors() -> (Option<Color>, Option<Color>) {
    (None, None)
}

/// Extract the color for a specific OSC code from a response buffer
/// that may contain multiple concatenated OSC replies.
fn parse_osc_response_for_code(response: &[u8], osc_code: u8) -> Option<Color> {
    let s = String::from_utf8_lossy(response);
    let marker = format!("]{};", osc_code);
    let start = s.find(&marker)?;
    let sub = &s[start..];
    // Find the end of this response (BEL or ST)
    let end = sub.find('\x07')
        .map(|i| i + 1)
        .or_else(|| {
            sub.find("\x1B\\").map(|i| i + 2)
        })?;
    parse_osc_color_response(sub[..end].as_bytes())
}
```

- [ ] **Step 3: Update `bg_color()` and `fg_color()` to use new unified probe**

Replace the separate `probe_osc_color` calls. Both functions now share one cache-initialization path:

Add a new `OnceLock` at the top of `src/terminal.rs` and a helper:

```rust
static PROBED_COLORS: OnceLock<(Option<Color>, Option<Color>)> = OnceLock::new();

fn probed_osc() -> &'static (Option<Color>, Option<Color>) {
    PROBED_COLORS.get_or_init(probe_osc_colors)
}
```

Then update `bg_color()`:

```rust
pub fn bg_color() -> Color {
    *BG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().1 {
            return c;
        }
        if let Some((_, bg)) = parse_colorfgbg() {
            return bg;
        }
        Color::new(0, 0, 0)
    })
}
```

And `fg_color()`:

```rust
pub fn fg_color() -> Color {
    *FG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().0 {
            return c;
        }
        if let Some((fg, _)) = parse_colorfgbg() {
            return fg;
        }
        Color::new(204, 204, 204)
    })
}
```

- [ ] **Step 4: Write tests for `parse_osc_response_for_code`**

Add to `mod tests`:

```rust
#[test]
fn parse_osc_response_for_code_extracts_fg() {
    // Concatenated fg + bg responses
    let combined = b"\x1B]10;rgb:cccc/cccc/cccc\x07\x1B]11;rgb:1c1c/1c1c/1c1c\x07";
    assert_eq!(
        parse_osc_response_for_code(combined, 10),
        Some(Color::new(0xcc, 0xcc, 0xcc))
    );
    assert_eq!(
        parse_osc_response_for_code(combined, 11),
        Some(Color::new(0x1c, 0x1c, 0x1c))
    );
}

#[test]
fn parse_osc_response_for_code_missing() {
    let only_bg = b"\x1B]11;rgb:0000/0000/0000\x07";
    assert_eq!(parse_osc_response_for_code(only_bg, 10), None);
    assert_eq!(
        parse_osc_response_for_code(only_bg, 11),
        Some(Color::new(0, 0, 0))
    );
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/terminal.rs
git commit -m "refactor: probe both fg/bg colors in a single /dev/tty session"
```

---

### Task 3: Add macOS system theme fallback

**Files:**
- Modify: `src/terminal.rs` (add `detect_macos_theme` function, wire into fallback chain)
- Test: `src/terminal.rs`

On macOS, `defaults read -globalDomain AppleInterfaceStyle` returns "Dark" when the system is in dark mode, or exits with an error in light mode. This runs in ~10ms and works even when OSC probing and COLORFGBG fail (e.g., in VS Code terminal, basic Terminal.app).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn macos_theme_fg_lighter_than_bg_in_dark_mode_or_vice_versa() {
    let (fg, bg) = system_theme_colors();
    // fg and bg should have distinct brightness — one light, one dark
    let fg_luma = fg.luma();
    let bg_luma = bg.luma();
    assert!((fg_luma - bg_luma).abs() > 0.3,
        "fg luma ({fg_luma}) and bg luma ({bg_luma}) should differ significantly");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test macos_theme -- --nocapture`
Expected: FAIL — function does not exist

- [ ] **Step 3: Implement `system_theme_colors` with caching**

Add a new `OnceLock` at the top of `src/terminal.rs`:

```rust
static SYSTEM_THEME: OnceLock<(Color, Color)> = OnceLock::new();
```

Then add the implementation:

```rust
/// Detect system theme and return conservative fg/bg color pair.
///
/// On macOS, queries `defaults read -globalDomain AppleInterfaceStyle`.
/// Cached in its own `OnceLock` so the command runs at most once.
fn system_theme_colors() -> (Color, Color) {
    *SYSTEM_THEME.get_or_init(detect_system_theme)
}

#[cfg(target_os = "macos")]
fn detect_system_theme() -> (Color, Color) {
    let is_dark = std::process::Command::new("defaults")
        .args(["read", "-globalDomain", "AppleInterfaceStyle"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            String::from_utf8(o.stdout).ok()
        } else {
            None
        })
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("dark"));

    if is_dark {
        (Color::new(204, 204, 204), Color::new(30, 30, 30))
    } else {
        (Color::new(51, 51, 51), Color::new(255, 255, 255))
    }
}

#[cfg(not(target_os = "macos"))]
fn detect_system_theme() -> (Color, Color) {
    // Non-macOS: assume dark theme (most developer terminals are dark)
    (Color::new(204, 204, 204), Color::new(0, 0, 0))
}
```

- [ ] **Step 4: Wire into the fallback chain**

Update `bg_color()` and `fg_color()` to use `system_theme_colors()` as the final fallback after `COLORFGBG`. Since `system_theme_colors()` is cached in its own `OnceLock`, `defaults read` runs at most once even though both `bg_color()` and `fg_color()` call it:

```rust
pub fn bg_color() -> Color {
    *BG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().1 {
            return c;
        }
        if let Some((_, bg)) = parse_colorfgbg() {
            return bg;
        }
        system_theme_colors().1
    })
}

pub fn fg_color() -> Color {
    *FG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().0 {
            return c;
        }
        if let Some((fg, _)) = parse_colorfgbg() {
            return fg;
        }
        system_theme_colors().0
    })
}
```

Note: on non-macOS, `detect_system_theme()` returns the same defaults as the current hardcoded values, so this is a no-op change for Linux/Windows.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/terminal.rs
git commit -m "feat: fall back to macOS system theme for color detection"
```

---

### Task 4: Add `is_light_theme()` and `is_dark_theme()` to the public API

**Files:**
- Modify: `src/terminal.rs` (add two new public functions)
- Modify: `src/lib.rs` (re-export)
- Test: `src/terminal.rs`

Users shouldn't need to manually compute luma from `bg_color()`. Give them a simple boolean API.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn theme_detection_is_consistent() {
    // is_light and is_dark should be mutually exclusive
    // (unless the background is exactly mid-gray, which is unlikely)
    let light = is_light_theme();
    let dark = is_dark_theme();
    assert!(light != dark);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test theme_detection -- --nocapture`
Expected: FAIL — functions do not exist

- [ ] **Step 3: Implement**

Add to `src/terminal.rs`:

```rust
/// Returns `true` if the terminal background appears light (luma > 0.5).
///
/// Uses the same detection chain as [`bg_color()`].
pub fn is_light_theme() -> bool {
    bg_color().luma() > 0.5
}

/// Returns `true` if the terminal background appears dark (luma ≤ 0.5).
///
/// Uses the same detection chain as [`bg_color()`].
pub fn is_dark_theme() -> bool {
    !is_light_theme()
}
```

- [ ] **Step 4: Update `src/lib.rs` re-exports**

Change the re-export line:

```rust
pub use terminal::{bg_color, fg_color, is_dark_theme, is_light_theme, probe_colors, set_bg_color, set_fg_color};
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/terminal.rs src/lib.rs
git commit -m "feat: add is_light_theme() / is_dark_theme() convenience API"
```

---

### Task 5: Add `TERM_PROGRAM` heuristics fallback

**Files:**
- Modify: `src/terminal.rs` (add `term_program_colors` function, wire into chain)
- Test: `src/terminal.rs`

Some well-known terminals have predictable default themes. When all other detection fails, use `TERM_PROGRAM` to make an educated guess instead of always assuming dark.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn term_program_heuristics() {
    assert_eq!(term_program_colors("Apple_Terminal"), Some((Color::new(0, 0, 0), Color::new(255, 255, 255))));
    assert_eq!(term_program_colors("iTerm.app"), None); // iTerm could be either
    assert_eq!(term_program_colors("unknown_terminal"), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test term_program -- --nocapture`
Expected: FAIL — function does not exist

- [ ] **Step 3: Implement `term_program_colors`**

Add to `src/terminal.rs`:

```rust
/// Guess fg/bg colors from the `TERM_PROGRAM` environment variable.
///
/// Only returns `Some` for terminals with a strong default theme.
/// Returns `None` for terminals that commonly vary (iTerm2, VS Code, etc.).
fn term_program_colors(term_program: &str) -> Option<(Color, Color)> {
    match term_program {
        // Apple Terminal defaults to white background
        "Apple_Terminal" => Some((Color::new(0, 0, 0), Color::new(255, 255, 255))),
        _ => None,
    }
}
```

- [ ] **Step 4: Wire into fallback chain**

Update `bg_color()` and `fg_color()`. Insert after `COLORFGBG` and before the macOS theme fallback:

```rust
pub fn bg_color() -> Color {
    *BG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().1 {
            return c;
        }
        if let Some((_, bg)) = parse_colorfgbg() {
            return bg;
        }
        if let Ok(tp) = std::env::var("TERM_PROGRAM") {
            if let Some((_, bg)) = term_program_colors(&tp) {
                return bg;
            }
        }
        system_theme_colors().1
    })
}

pub fn fg_color() -> Color {
    *FG_COLOR.get_or_init(|| {
        if let Some(c) = probed_osc().0 {
            return c;
        }
        if let Some((fg, _)) = parse_colorfgbg() {
            return fg;
        }
        if let Ok(tp) = std::env::var("TERM_PROGRAM") {
            if let Some((fg, _)) = term_program_colors(&tp) {
                return fg;
            }
        }
        system_theme_colors().0
    })
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/terminal.rs
git commit -m "feat: add TERM_PROGRAM heuristics for terminal color fallback"
```

---

### Task 6: Update doc comments for new fallback chain

**Files:**
- Modify: `src/terminal.rs` (update doc comments on `probe_colors`, `bg_color`, `fg_color`)

- [ ] **Step 1: Update the doc comments**

Update `probe_colors`:
```rust
/// Eagerly probe and cache both terminal colors.
///
/// Detection chain (first match wins):
/// 1. OSC 10/11 query via `/dev/tty` (most accurate — returns exact RGB)
/// 2. `COLORFGBG` environment variable (ANSI index → approximate RGB)
/// 3. `TERM_PROGRAM` heuristics (known default themes)
/// 4. System theme detection — macOS `defaults read` (dark/light → conservative colors)
/// 5. Hardcoded defaults (light gray fg, black bg — non-macOS only)
///
/// Call this before hiding the cursor or writing any escape sequences,
/// because the OSC probe reads from the terminal and stray escape output
/// could corrupt the response. Safe to call multiple times — only probes
/// on first call.
```

Update `bg_color`:
```rust
/// Get the terminal background color.
///
/// Uses the detection chain described in [`probe_colors`]. Caches the result.
```

Update `fg_color`:
```rust
/// Get the terminal foreground color.
///
/// Uses the detection chain described in [`probe_colors`]. Caches the result.
```

- [ ] **Step 2: Run doc-tests**

Run: `cargo test --doc`
Expected: All doc-tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/terminal.rs
git commit -m "docs: document the full color detection fallback chain"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All unit tests and doc-tests PASS

- [ ] **Step 2: Build all examples**

Run: `cargo build --examples`
Expected: All examples compile

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 4: Verify the public API**

Check that `src/lib.rs` re-exports everything:
- `bg_color`, `fg_color`, `probe_colors`, `set_bg_color`, `set_fg_color`
- `is_light_theme`, `is_dark_theme` (new)

- [ ] **Step 5: Final commit if needed**

```bash
git add -A
git commit -m "chore: final cleanup after terminal detection improvements"
```
