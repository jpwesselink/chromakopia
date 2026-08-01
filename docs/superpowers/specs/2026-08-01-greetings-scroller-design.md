# Design: `examples/greetings.rs` — cracktro greetings scroller

**Date:** 2026-08-01
**Status:** Approved

## Goal

A demoscene-style greetings scroller: a long plain-text message scrolls
across mid-screen, and when the message reaches an inline cue marker,
name marquees pop in above and below it, bouncing.

## Message

- One `const MESSAGE: &str` — single-line plain text with inline `{cueN}`
  markers. Plain text is the point: the marquee loop width is the string
  length, so messages can be arbitrarily long and edited freely.
- Rendered as `Marquee::wrap(Rainbow::on(...))` at 16 cells/s. Reading
  speed caps this row (legibility trumps the fast-ish house rule here).
- The message gets trailing-space padding so its loop has a breather
  before repeating.

## Cue timing

- A helper parses `MESSAGE`: returns the display text with `{cueN}`
  markers stripped, plus each cue's character index in the display text.
- Cue N fires when its character reaches screen center:
  `t = max(0, idx − term_width/2) / speed` seconds.
- Moving a marker in the string moves the timing; nothing is hardcoded.

## Greetings

At cue 1, three name rows appear, staggered 0.4s apart, in this vertical
order (message in the middle):

- above: `super_jo_nathan`
- below: `Robbert-Jan`
- below that: `janglad`

Each row is:

```rust
DelayedStart::new(cue1 + stagger,
    VSineWave::wrap(Marquee::wrap(<colored name>).speed(...).direction(...))
        .amplitude(1.0))
```

- `DelayedStart` blanks the row until its moment, then starts the inner
  effect from frame 0 — the "bounce in at cue" behavior.
- Alternating scroll directions (Left / Right / Left) and varied speeds.
- Each name string has one blank pad row above and below (headroom for
  the ±1-cell bounce) and trailing spaces for the loop gap.
- Distinct colorings per name (e.g. Glow / solid accent / Rainbow).

## Layout & runtime

- Plain `Scene` stack: name row, blank, message row, blank, name row,
  name row. Header `text()` line naming the demo and `q` to quit.
- `.quit_key('q').run(total)` where
  `total = display_len / 16.0 + ~4.0` seconds — the full message is
  always seen at least once.
- House style: doc-comment header with run command, prelude import,
  `#[tokio::main]`.

## Testing

- `cargo check --examples`.
- Unit-style assertion in the example is overkill; the cue parser is a
  ~10-line pure function — verified by a debug `assert!` that parsing
  found cue 1 (panics early if the marker is missing).
- Pty smoke run: renders, `q` quits.

## Out of scope

- Library changes (a bounce-entrance / SlideIn effect stays an audit
  item for later if the pop-in feels abrupt).
- Figlet rendering of the message or names.
