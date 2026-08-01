# Design: opt-in `.quit_key(char)` for animations

**Date:** 2026-08-01
**Status:** Approved

## Goal

Every animated demo example should quit when the user presses `q`. The
mechanism is an opt-in builder method on the animation API, not a default
behavior of the runtime.

## API

New builder method `quit_key(char)` on the three types that expose
`.run(seconds)` / `.spawn()`:

- `Scene` (`src/animate/scene.rs`) — `scene.quit_key('q').run(15.0)`
- `On<E>` (`src/animate/framebuffer.rs`) — `Plasma::on("…").quit_key('q').spawn()`
- macro-generated `run`/`spawn` on bare effects (`src/animate/effects.rs`) —
  `HeightMap::new().quit_key('q').run(30.0)`

Each stores `quit_key: Option<char>` (default `None`, existing behavior
unchanged) and forwards it to `run_effect()` / `spawn_effect()`, which grow a
`quit_key: Option<char>` parameter.

## Runtime behavior

When a quit key is set and the process is on a real terminal (unix, colors
enabled):

- The runtime spawns one background thread that opens `/dev/tty`, disables
  echo and line-buffering via termios — keeping `ISIG` so Ctrl-C still works —
  and polls for input with a ~100 ms timeout (`VMIN=0`, `VTIME=1`) while the
  `running` flag is true.
- On a case-insensitive key match it flips `running` to false. Existing
  cleanup paths restore the cursor as they do today.
- A drop guard restores the original termios when the thread exits (same
  pattern as `probe_osc_colors` in `src/terminal.rs`).
- In `run_effect`, the fixed `sleep(duration)` becomes a deadline loop so a
  keypress ends the run early instead of waiting out the clock.

Non-unix and CI/no-color paths: no listener is spawned; `.quit_key()` is a
harmless no-op.

## Examples

Add `.quit_key('q')` to all animated examples (`demo.rs` is static output and
is skipped). `plasma.rs` and `rainbow.rs` spawn then sleep; their sleep
becomes a race against `handle.wait()` so `q` ends the demo promptly.

## Testing

- Unit tests for the tty-free parts: builder plumbing stores the key;
  `None` spawns no listener thread.
- The tty interaction (raw-mode read, early exit, termios restore) is
  verified manually by running a demo — CI has no tty.

## Out of scope

- Esc / Ctrl-C as additional quit keys (Ctrl-C already works via ISIG).
- Any on-screen "press q to quit" hint.
- Windows key handling.
