# Design: upstream `VSineWave` + wavy marquee demo row

**Date:** 2026-08-01
**Status:** Approved

## Goal

Move `letters_drop.rs`'s private `VSineWave` wrapper into the library and
show it off with a sixth row in `examples/marquee.rs` — a ribbon that
scrolls horizontally while rippling vertically.

## API

New public effect in `src/animate/effects.rs`, named after `Marquee`'s
constructor convention:

```rust
VSineWave::wrap(inner)      // any Effect
    .amplitude(4.0)         // peak vertical displacement, cells (default 4.0)
    .frequency(0.15)        // radians per column (default 0.15)
    .speed(0.05)            // radians per frame (default 0.05)
```

Semantics are a direct port of the example struct: render the inner effect
into a scratch buffer sized to the output, then shift each column by
`round(sin(x * frequency + frame * speed) * amplitude)` cells; skip space
cells; write only into empty target cells; drop cells displaced outside
the buffer. `size()` forwards `inner.size()` (like `Marquee`).

Defaults are the example's constructor values.

## Exports

- `VSineWave` added to `pub use effects::{...}` in `src/animate/mod.rs`.
- `VSineWave` **and `Phases`** added to `crate::prelude` (Phases was left
  out of the prelude when it landed — housekeeping rider).

## Migration

`examples/letters_drop.rs` deletes its `VSineWave` struct + builders +
`impl Effect` and calls `VSineWave::wrap(...)` instead of
`VSineWave::new(...)`; the `.amplitude(10.0).frequency(0.04).speed(0.10)`
chain is unchanged. Behavior-identical.

## Demo row

`examples/marquee.rs` gains a sixth section:

- Content: the existing ribbon padded to 5 lines (`"\n\n" + RIBBON + "\n\n"`
  as a const) so the wave has ±2 rows of headroom around the center line.
- `VSineWave::wrap(Marquee::wrap(Rainbow::on(RIBBON_WAVE)).speed(8.0))
  .amplitude(2.0).frequency(0.12).speed(0.08)`
- Label: `6. scroll + VSineWave — the ripple marquee`.

## Testing

Unit tests in `src/animate/effects.rs`:

- Zero amplitude → passthrough (char stays at its source cell).
- Known displacement: `frequency(0.0).speed(FRAC_PI_2).amplitude(2.0)` at
  frame 1 gives `sin = 1.0` → a char at row 0 lands at row 2 in a 5-row
  buffer, and row 0 is empty.
- Plus `cargo check --examples` and a pty smoke run of `marquee`.

## Out of scope

- Horizontal sine displacement (no current consumer).
- Changing `VSineWave`'s per-frame units to seconds (matches sibling
  effects like `Plasma.speed`; behavior-identical migration wins).
