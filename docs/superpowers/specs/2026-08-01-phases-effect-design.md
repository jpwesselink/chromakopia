# Design: `Phases` effect — sequential phases with continuous frames and crossfades

**Date:** 2026-08-01
**Status:** Approved

## Problem

Timeline tracks give each effect a local frame counter that resets to 0 at
window start, and `Chain` does the same per link. A continuous effect that
should span sequential phase boundaries (e.g. Neon running while the scene
moves from a Glow phase into a Neon phase) visibly "jumps" when its counter
resets. `examples/letters_drop.rs` works around this with a private
`PhaseTransitions` effect. That logic belongs in the library.

## API

New public effect `Phases` in `src/animate/effects.rs`:

```rust
Phases::new()
    .at(0.0, Solid(black))   // phase starts at 0s, relative to the effect's frame 0
    .at(1.0, Glow::new())
    .at(6.5, Neon::new())
    .crossfade(1.0)          // seconds; default 0.0 = hard cut
```

- `.at(seconds, effect)` — adds a phase starting at an absolute time within
  the effect's life. Phases are kept sorted by start frame on insertion, so
  call order cannot break phase selection.
- `.crossfade(seconds)` — blend duration applied at the start of every phase
  after the first.

## Semantics

- The active phase is the last one whose start frame ≤ the current frame.
- Every inner effect renders with the frame `Phases` itself receives —
  inside a Timeline track that is the track-local frame, continuous across
  all phase boundaries. No per-phase resets.
- During the first `crossfade` seconds of a phase (other than the first),
  the previous phase renders into a scratch buffer holding the same chars
  with colors reset to `DEFAULT_TEXT_COLOR` (so it colors from a clean
  slate), and cell colors lerp previous → active with
  `t = (frame - start) / fade_frames`. Whitespace cells are skipped.
- Empty `Phases` renders nothing. `size()` is `(0, 0)` — it is a color
  transform, like `Solid`.
- Direct port of `PhaseTransitions::render` from `examples/letters_drop.rs`,
  with a seconds-based builder per project convention.

## Exports

Add `Phases` to the `pub use effects::{...}` list in `src/animate/mod.rs`
(same list as `Chain`, `Transition`).

## Migration

`examples/letters_drop.rs` deletes its `PhaseTransitions` struct and
`impl Effect` (~50 lines) and rewrites the call site as the builder chain
above using its existing `BG_*` timing constants. Behavior must be
visually identical.

## Testing

Unit tests in `src/animate/effects.rs`:

- Phase selection with `Solid` phases: before phase 2 starts → color A;
  after `start + fade` → color B.
- Mid-crossfade → color strictly between A and B.
- Frame continuity: a probe effect that encodes the received frame number
  into a color channel; assert phase 2's render receives the global frame,
  not a phase-local one.
- Out-of-order `.at()` calls select the same phases as sorted calls.

## Out of scope

- Changes to `Chain`, `Transition`, or `Timeline` — their reset semantics
  stay as documented.
- Per-phase easing on the crossfade (linear only, like the example).
