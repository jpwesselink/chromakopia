# Wind Effect — Design

**Status:** Approved 2026-05-04
**Branch:** feat/framebuffer-renderer
**Author:** brainstormed with JP

## Summary

A new `Wind` effect for the chromakopia animation library. Wraps an inner `Effect` and, over a configurable duration, dissolves its output as if the rendered text were a cloth caught in a building gust. The cloth flutters in coherent waves, then tears free starting from the trailing (upwind) edge while the wind blows the released cells off-screen and they fade out.

A one-shot transition: at `t = 0` the inner effect renders normally; by `t = duration + fade_tail` the buffer is empty.

## Motivation

Existing dissolves in the library are either fades (no motion) or directional scrolls (no decay). Nothing produces the "blown away" look that's a staple of demoscene intros. This effect plugs the gap and composes with every existing color effect (`Rainbow`, `Plasma`, `Glow`, …) via the same wrapper pattern as `Fld` / `FadeEnvelope` / `DelayedStart`.

## Non-goals

- Not a true mass-spring cloth simulation. The look comes from a coherent noise-displacement field, not from springs. (Rejected explicitly during brainstorming — see "Approach" section.)
- Not a looping ambient effect. One-shot only. A reverse / "fly-in" variant is out of scope.
- Not a fluid simulation. Wind is a scalar magnitude × fixed direction, ramped in time.

## Approach

Three independent sub-models, all driven by elapsed time `t` (seconds since effect start):

1. **Flutter field** — coherent 2D displacement applied to every still-attached cell. Implemented as a sum of sines (cheap, deterministic, no Perlin dependency):
   - `dx(c, r, t) = flutter_amp · [ sin(c·flutter_freq + t·flutter_speed) + sin((c + r)·0.4·flutter_freq + t·flutter_speed·1.3) ]`
   - `dy(c, r, t)` analogous with different phases.
   - Adjacent cells differ by tiny phase shifts → look woven, not noisy.

2. **Wind ramp** — global wind vector that builds over the first 70% of the duration:
   - `wind_mag(t) = strength · smoothstep(0, 0.7·duration, t)²`
   - `wind_vec(t) = wind_mag(t) · (cos θ, sin θ)`  where `θ = angle_rad`.
   - Squaring the smoothstep gives a slow start and a sharper final pull.

3. **Per-cell release time** `τ(c, r)` — scalar field. Trailing (upwind) edge releases first.
   - `proj = (c·cos θ + r·sin θ) / max_proj` where `max_proj = width·|cos θ| + height·|sin θ|`. Yields `proj ∈ [0, 1]`, `0` = trailing.
   - `τ(c, r) = duration · (0.1 + 0.7·proj + 0.15·release_noise(c, r))`
   - `release_noise` is a stateless hash-based pseudo-random per cell (no `rand` calls during rendering — same `(c, r)` always returns the same value). Gives a ragged tear front.

### Per-cell behavior

For each cell `(c, r)` non-empty in the inner effect's output:

- **`t < τ(c, r)`** — attached. Render at `(c + dx, r + dy)` with full alpha, original character & color.
- **`t ≥ τ(c, r)`** — released. Free-flying particle:
  - Initial velocity at release = analytic time-derivative of the flutter field at `τ` plus a small wind impulse.
  - Forward Euler each subsequent frame (computed analytically from `t − τ`, no per-cell stored state):
    - `v(Δ) = v₀·exp(−damping·Δ) + (wind_vec/damping)·(1 − exp(−damping·Δ))`
    - `pos(Δ) = c + ∫v(Δ′)dΔ′` — closed-form integral of the above.
  - Alpha decays linearly: `α(Δ) = max(0, 1 − Δ / fade_tail)`.

The closed-form integration is the win of the noise-field approach: no per-cell state means time-scrubbing works (frame N can be rendered without rendering frame N−1 first), tests are deterministic, and the effect plays nicely with `On<E>::frame()`.

### Aspect-ratio correction

Terminal cells are visually ~2:1 (rows taller than cols are wide). All `dy` values and the y-component of `wind_vec` are multiplied by `0.5` before being applied to the framebuffer. Without this, diagonal wind looks ~2× too steep. Bake this into the constant `Y_ASPECT = 0.5` at the top of the module.

## Public API

```rust
pub struct Wind {
    inner: Box<dyn Effect>,
    duration: f64,
    angle_rad: f64,
    strength: f64,
    flutter_amp: f64,
    flutter_freq: f64,
    flutter_speed: f64,
    damping: f64,
    fade_tail: f64,
}

impl Wind {
    pub fn new<E: Effect + 'static>(inner: E) -> Self;     // sane defaults below
    pub fn duration(self, seconds: f64) -> Self;
    pub fn angle_deg(self, deg: f64) -> Self;              // convenience
    pub fn angle_rad(self, rad: f64) -> Self;
    pub fn strength(self, cells_per_second: f64) -> Self;
    pub fn flutter(self, amp: f64, freq: f64, speed: f64) -> Self;
    pub fn damping(self, d: f64) -> Self;
    pub fn fade_tail(self, seconds: f64) -> Self;
}

impl Effect for Wind {
    fn render(&self, buf: &mut FrameBuffer, frame: usize);
}
```

### Default values

| Field | Default | Rationale |
|---|---|---|
| `duration` | `2.5` | Matches the median scene length in existing demos. |
| `angle_deg` | `15.0` | Slight upward angle reads as "wind" not "gravity". |
| `strength` | `30.0` | Cells/s. Carries a 90-col release across in ~3s combined with damping. |
| `flutter_amp` | `0.6` | Just under one cell — visible jitter, no smearing. |
| `flutter_freq` | `0.25` | Wavelength ~25 cells; longer than the text width so the whole block waves together. |
| `flutter_speed` | `2.0` | Two oscillations per second — readable flutter. |
| `damping` | `1.5` | Released particles approach terminal velocity in ~0.7s. |
| `fade_tail` | `0.8` | Particles fully gone within ~1s of release. |

## Rendering algorithm

`fn render(&self, out: &mut FrameBuffer, frame: usize)` does:

1. Determine `t = frame as f64 / FPS` where FPS comes from the renderer's frame rate. Verify during implementation whether `FrameBuffer` carries this; if not, take it as a `const` in this module documented to match the renderer.
2. Allocate a temp `FrameBuffer` of the same size as `out`.
3. Call `self.inner.render(&mut temp, frame)`.
4. Clear `out` (to whatever the framebuffer treats as empty/background).
5. For each non-empty cell `(c, r)` in `temp`:
   - Compute `τ` for `(c, r)`.
   - If `t < τ`: compute `(dx, dy)` from flutter, alpha = 1.
   - Else: compute `(dx, dy)` from analytic free-flight integral, alpha from `fade_tail`.
   - Skip if `α ≤ 0`.
   - Compute target cell `(tc, tr) = (round(c + dx), round(r + dy · Y_ASPECT))`.
   - Skip if `(tc, tr)` is outside the framebuffer.
   - Sample inner color at `(c, r)`; dim by α (lerp toward background).
   - Write character + dimmed color to `out[tc, tr]`. **Collision rule:** highest-α wins.

Steps 4–5 require either a tiny scratch field for tracking max-α-per-output-cell, or a two-pass loop. Either works; pick the one that keeps the hot inner loop simple.

## Composability & integration

- Plugs into `Scene` and `On<E>` like any other effect.
- Composes with `text(...)`, `Solid`, `Rainbow`, `Plasma`, `Glow`, `Spread`, etc., as the inner.
- For multi-line text: the effect treats the whole text block as one cloth (no per-line independence). This is the requested behavior.
- The effect is CI-safe automatically because it's pure — its output goes through the same `to_string_auto()` path as everything else.

## Determinism & testing

- No `rand::*` calls during `render`. The release-time noise is a hash of `(c, r, seed)` where `seed` is a field on `Wind` (default `0`, settable via `.seed(u64)` if we want).
- Same `(width, height, t)` tuple always produces the same buffer. Easy to write golden-frame tests.

### Test plan

- **Unit**: a small inner effect (e.g., `Solid` over a 10×3 area) wrapped in `Wind`. Snapshot at `t = 0`, `t = 0.5·duration`, `t = duration`, `t = duration + fade_tail`. Assert:
  - At `t = 0`: every input cell is rendered (possibly displaced by `flutter_amp`).
  - At `t = duration + fade_tail + 0.1`: buffer is empty.
  - At each intermediate `t`: count of non-empty cells is monotonically non-increasing across frames.
- **Property**: `wind.render(buf, frame)` does not panic for `frame ∈ {0, 1, …, FPS·(duration+fade_tail+1)}` across a few buffer sizes.
- **Visual**: add a "Wind" demo scene in `chromakopia-wasm/src/lib.rs` so you can eyeball it in the browser demo.

## Risks & open questions

- **Source of `t` from `frame: usize`.** Verify during implementation whether the FrameBuffer carries an FPS or a `seconds_per_frame`. If not, hard-code a const matching the renderer and document it in the impl.
- **Collision policy.** "Highest-α wins" is correct but slightly more code than "first-write-wins". If profile shows the scratch-field allocation is a hot path, fall back to first-write-wins — visually nearly identical for this effect.
- **Wind direction tied to text orientation.** A vertical wind (`angle_deg = 90`) on a single-line banner will look weird because the cloth is one row tall. Document this; don't fix it. (User is unlikely to do this on purpose.)

## Out of scope (deferred)

- Reverse mode (assembly from particles) — separate feature, separate spec.
- Per-line cloth (each row tears independently) — would require per-cell state; revisit if requested.
- True spring-cloth simulation — explicitly rejected during brainstorming.
- Configurable particle glyph substitution mid-flight — could be a follow-up.

## Files affected

- `src/animate/effects.rs` — add `Wind` struct + `impl Effect`.
- `src/animate/mod.rs` — re-export `Wind`.
- `src/lib.rs` — add `Wind` to `prelude`.
- `chromakopia-wasm/src/lib.rs` — new demo scene.
- `web/docs/demo/index.mdx` — add row to effect-showcase table.
- Tests inline in `effects.rs` (matches existing convention).
