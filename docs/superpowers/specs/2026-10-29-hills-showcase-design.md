# Hills showcase — design

Turn `examples/hills` into an endless cracktro-style showcase of every
chromakopia effect, played out over the pastel hills landscape.

## Goal

One demo that walks through everything the library can do, block by
block, each effect presented on its own styled panel with a caption —
skippable with the keyboard, looping forever until `q`.

## Storyboard

The pastel landscape (sky, clouds, hills, cloud shadows) and the licence
scroller along the bottom run throughout. Everything else is sequenced.

| # | Block | What happens | Duration |
|---|-------|--------------|----------|
| 1 | Intro | CHROMAKOPIA drops in letter by letter (existing bounce + DYCP); licence scroller starts | ~7 s |
| — | Logo exit | Letters jump up out of the screen one by one, same order as they arrived | ~1.5 s |
| 2 | COLOR EFFECTS | Heading drops in; per effect a panel bounces in, holds, falls out | 7 × 5 s |
| 3 | SHAPE & LAYOUT | Same pattern | 9 × 5 s |
| 4 | FIELDS | Larger panels | 2 × 6 s |
| 5 | MIXING | Blend modes and a transition | 3 × 5 s |
| 6 | Finale | Panel and heading leave; CHROMAKOPIA drops in again; then loop to block 2 | ~6 s |

### Panels and captions

| Block | Effect (panel content: its name) | Subtitle |
|-------|----------------------------------|----------|
| 2 | Plasma | smooth as butter |
| 2 | Rainbow | every hue, all the time |
| 2 | Glow | a spotlight with a gradient |
| 2 | Pulse | heartbeat from the centre |
| 2 | Neon | buzzing like a bar sign |
| 2 | Radar | sweep, sweep, ping |
| 2 | Karaoke | sing along, one letter at a time |
| 3 | Glitch | signal lost |
| 3 | Flap | departures, now boarding |
| 3 | Scroll | sliding into place |
| 3 | Spread | fanning out from one line |
| 3 | Marquee | round and round |
| 3 | VSineWave | riding the wave |
| 3 | Dycp | every letter its own height |
| 3 | Fld | rows floating free |
| 3 | Wind | blown away |
| 4 | VectorField | follow the flow |
| 4 | HeightMap | hills you can read |
| 5 | Plasma × Radar (`Screen`) | light on light |
| 5 | Rainbow × Glow (`Multiply`) | shadows with colour |
| 5 | Transition Plasma → Rainbow | from one to the other |

### Look

- **Panel:** a rectangle centred where the logo sat, dark aubergine fill
  (`#2A1638`, the letter-outline colour), one-pixel dark outline and the
  same two-step violet 3D extrusion down-right as the letters. Same size
  for every panel in a block (block 4 larger).
- **Panel content:** the effect's name in figlet Colossal (Basic when
  Colossal would not fit), driven by that effect. Empty cells keep the
  aubergine fill.
- **Heading and subtitle:** C64-ROM pixel font in the letter style
  (`stamp`), heading above the panel, subtitle below.
- **Panel motion:** drops in from above with `BounceOut` (~0.7 s), holds,
  falls out downward with `EaseIn` (~0.5 s). Panel y is always a whole
  cell, so its pixel frame and cell content never drift half a row apart.
- **Logo exit:** reverse of the entrance — each letter, staggered, eases
  up and off the top.

### Keys

| Key | Action |
|-----|--------|
| `→`, `n`, space | next panel (or next step) |
| `←`, `p` | previous panel |
| `↓` | next block |
| `↑` | previous block |
| `1`–`6` | jump to block |
| `q` | quit; alternate screen restores the terminal |

## Architecture

Approach A: the pixel world and cell effects are two passes over the same
frame. Cell effects keep their real glyphs; nothing is converted.

### Library: key input

- `pub enum Key { Char(char), Left, Right, Up, Down }`.
- `Scene::on_key(impl Fn(Key) + Send + Sync + 'static)` — every key goes
  to the handler. `quit_key` keeps working as now.
- The existing `/dev/tty` listener reads keys whenever `quit_key` or
  `on_key` is set; arrow keys are `ESC [ A/B/C/D`.
- Byte → `Key` decoding is a pure function `parse_keys(&[u8]) -> Vec<Key>`
  (unknown escape sequences dropped), unit-tested.

### Example layout

`examples/hills.rs` (~500 lines, heading for ~1000) becomes a directory;
Cargo picks up `examples/hills/main.rs` as example `hills`.

| File | Contents |
|------|----------|
| `main.rs` | wiring: layers, stage, scene, keys |
| `landscape.rs` | sky, clouds, ridges, cloud shadows (existing code) |
| `glyphs.rs` | Mario glyphs, C64 glyphs, `stamp()` |
| `figlet.rs` | `.flf` loader (copied from `greetings.rs`, which stays as is) |
| `stage.rs` | step list, `locate`, seek requests, per-frame view |

### Stage

- A step list: `Intro`, `LogoExit`, `Heading(block)`, `Panel { name,
  subtitle, effect }`, `Finale`, each with a duration.
- `locate(t) -> (step index, local time)` is pure; all animation reads
  local time, so every panel effect starts at its frame 0.
- Keys don't touch state directly: the handler stores a seek request
  (`Next`, `Prev`, `NextBlock`, `PrevBlock`, `Block(n)`) in an
  `Arc<Mutex<Option<Seek>>>`; the stage applies it at the start of the
  next frame by moving its time origin to the target step's start.
- After the finale the origin jumps back to block 2's start (loop).

### Per frame

1. Stage applies any pending seek, locates the step, and publishes a
   `View` (logo letter offsets, panel y, heading, subtitle) to an
   `Arc<Mutex<View>>`.
2. `Layers` renders the pixel world. Logo, panel frame, heading and
   subtitle are paint layers that read the `View`.
3. The active panel effect renders into a buffer the size of the panel
   interior (local frame), and is copied over the panel's cells: glyph
   cells with aubergine bg, blank cells aubergine.

### Error handling

- Missing glyph for a caption character: `expect` at startup (captions
  are fixed strings — a missing glyph is a programming error caught on
  first run, not at frame time).
- Terminal narrower than a panel: panel width clamps to screen width
  minus margins; names fall back from Colossal to Basic.

## Testing

- **Library:** `parse_keys` — plain chars, arrows, mixed input, unknown
  escape sequences.
- **Stage** (`cargo test --example hills`): `locate` on step boundaries;
  `Next`/`Prev` seeks; block jumps; loop after the finale.
- **Performance:** benchmark every block stays well under 16.7 ms/frame
  at 200 columns.
- **Visual:** one rendered frame per block, checked by eye.

## Out of scope

- Moving the figlet loader or glyph stamping into the library.
- Sprites, day/night, copper bars, glint — possible later blocks.
