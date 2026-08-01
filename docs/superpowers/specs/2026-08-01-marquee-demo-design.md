# Design: `examples/marquee.rs` — dedicated Marquee demo

**Date:** 2026-08-01
**Status:** Approved

## Goal

A single example showing off Marquee's own API — the features no existing
example demonstrates: `direction()`, `cycle(seconds, easing)`, `swing()`,
and the `size()` window for vertical crawls. The two parallax examples use
Marquee only as constant-speed left-scrolling layers.

## Layout

One `Scene`, five labeled sections, dim-gray `text()` labels with
`.blank()` spacing, header noting `q` quits:

1. **Scroll left (default)** — star-separated ribbon
   `" CHROMAKOPIA ★ TERMINAL ANIMATIONS ★ "` wrapped in `Rainbow`,
   `.speed(10.0)`. The zero-config case.
2. **Scroll right** — same ribbon colored by `Plasma` with a fire palette,
   `.direction(ScrollDirection::Right).speed(6.0)`.
3. **Eased cycle** — box-drawing strip (`═══╡ chromakopia ╞═══` style)
   with `.cycle(2.0, Easing::EaseOut)`: decelerating swipes.
4. **Swing** — `.swing().cycle(5.0, Easing::Linear)` on a `Glow`-colored
   line; amplitude = speed × cycle / 4, so `.speed(12.0)` gives ±15 cells.
   (`swing()` without `cycle()` is inert — the demo must pair them.)
5. **Vertical crawl** — a 6-line credits block scrolled
   `.direction(ScrollDirection::Top)` through a 3-row window via
   `.size(width, 3)`.

## Conventions

- House style of `vector_field.rs`: doc-comment header with run command,
  `use chromakopia::prelude::*`, `#[tokio::main]`,
  `.quit_key('q').run(30.0)`.
- Add one line to the README examples list:
  `cargo run --release --example marquee   # every Marquee mode, labeled`.

## Testing

- `cargo check --examples` passes.
- Pty smoke run: renders, `q` quits early.

## Out of scope

- Library changes of any kind.
- `ScrollDirection::Bottom` (Top shows the vertical path; four rows of
  direction permutations adds length, not insight).
