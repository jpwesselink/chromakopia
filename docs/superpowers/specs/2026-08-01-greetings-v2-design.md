# Design: greetings scroller v2 — colossal message, choreographed name drops

**Date:** 2026-08-01
**Status:** Approved

## Goal

Rework `examples/greetings.rs`: the long message renders in figlet
**colossal** and scrolls horizontally, later picking up a sine wave; at
`{cue1}`, ten names in figlet **basic** drop in one at a time from the top
right with a bounce-land / dwell / hop / exit-down choreography, each new
name starting when the previous sequence is halfway done.

## Names (in order)

super_jo_nathan, Robbert-Jan, janglad, Noodlelover, NOTHELPFUL, Dillygirl,
Leonrat, KBMedia, DocWilco, Yingeling. (Original three kept; user can trim.)

## Library change: `VSineWave` amplitude envelope

- New builders: `.delay(seconds)` — wave amplitude is 0 until then;
  `.ramp(seconds)` — amplitude then eases linearly 0 → full over this span.
- Defaults `0.0`/`0.0` preserve existing behavior exactly (ramp 0 = step).
- Envelope multiplies `amplitude` in `render`; wave phase still uses the
  global frame.
- Unit tests: passthrough before delay; full displacement after
  delay + ramp; half displacement mid-ramp.

## Figlet at runtime (example-local, XBin-loader precedent)

- Copy `colossal.flf` and `basic.flf` from
  `/opt/homebrew/share/figlet/fonts/` into `examples/fonts/`; load via
  `include_str!`. No runtime dependency on the figlet CLI.
- `FigletFont::parse(&str)`: header gives hardblank (last char of the
  `flf2a$` signature), height (field 2), comment-line count (field 6);
  skip comments; then 95 glyphs (ASCII 32–126), `height` lines each;
  strip `\r`, strip trailing endmark chars (last char of each line),
  replace hardblank with space.
- `render(&str) -> Vec<String>` and `width(&str) -> usize`: full-width
  layout (no smushing — letters stand alone, per standing preference);
  glyph width = max row width, short rows padded.
- Unknown chars (outside ASCII 32–126) render as space (glyph 0).

## Message row

- `MESSAGE` stays a plain editable string with `{cue1}` marker;
  `parse_cues` (kept from v1) strips markers and records char indices.
- Cue time computed in rendered columns:
  `cue_col = font.width(display_text[..cue_idx])`;
  `t = max(0, cue_col − term_w/2) / SPEED`.
- Colossal render (11 rows) + 2 blank pad rows top and bottom = 15-row
  block, wrapped `VSineWave(Marquee(Rainbow::on(...)))`.
- `SPEED = 60.0` cells/s (2 cells/frame — big glyphs read fast).
- Wave: `amplitude 2.0, frequency 0.05` (~126-col wavelength),
  `speed 0.075` (crests 45 cells/s, at the strobe ceiling),
  `.delay(8.0).ramp(2.0)` — flat scroll first, wave fades in at 8s.

## Name choreography

One example-local `NamesSequence` effect, added as a full-height
`Scene::overlay` (added first in the chain → anchored at row 0; overlays
composite after the stack and consume no layout space). Per name `i`,
start = `cue1 + i × D/2` (next starts when previous is halfway done),
with per-name local time driving four phases (D ≈ 4.8s total):

1. **descend** (2.2s): y from above the top edge to the bottom landing
   row, `Easing::BounceOut`; x anchored right
   (`buf.width − name_width − 2`, clamped ≥ 0); per-column sine wobble
   (±1 cell) that fades out as the bounce lands.
2. **dwell** (1.2s): rest at the landing row.
3. **hop** (0.6s): `y − 2·sin(π·u)` — small rise and settle.
4. **exit** (0.8s): accelerate down off-screen (`Easing::EaseIn`).

Names colored from a small cycling accent palette; non-space cells only.

## Layout

Scene: names overlay (full height) → header `text()` → 15-row message
block → 4 blank rows (landing zone). `.quit_key('q')`,
`total = rendered_message_width / SPEED + names_tail`, where names_tail
covers `cue1 + 10 × D/2 + D` if that exceeds the message pass.

## Testing

- `cargo test` — VSineWave envelope tests (library).
- Example asserts: `{cue1}` found; both fonts parse to 95 glyphs of the
  declared height (debug `assert!`s at startup).
- `cargo check --examples`; pty smoke run past the cue, fragments of
  several names present; `q` quits.

## Out of scope

- Smushing/kerning in the figlet renderer.
- Wiring `{cue2}`+ (parsed but unused).
- Promoting the figlet parser to the library (candidate for later).
