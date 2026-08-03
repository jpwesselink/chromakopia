//! Greetings v2 — a cracktro-style scroller.
//!
//! The message renders in figlet Colossal at startup (fonts are embedded
//! `.flf` files, parsed by the loader below — no figlet CLI needed) and
//! scrolls flat; 8 seconds in, a sine wave ramps up under it. When the
//! `{cue1}` marker scrolls past screen center, ten names in figlet Basic
//! drop in from the top right one by one: bounce-land at the bottom,
//! dwell, hop, exit down — each new name starting when the previous
//! sequence is halfway done.
//!
//! Run: `cargo run --release --example greetings`   (press q to quit)

use chromakopia::animate::framebuffer::{secs_to_frames, tempo};
use chromakopia::animate::Cell;
use chromakopia::prelude::*;

const MESSAGE: &str = "Hey, thanks for watching. I'd like to thank some people for \
congratulating me for my birthday.{cue1} And now that everyone is here, let's talk \
chromakopia: terminal animations, plain characters, no pixels harmed. \
Thanks for scrolling by!";

const NAMES: [&str; 10] = [
    "super_jo_nathan",
    "Robbert-Jan",
    "janglad",
    "Noodlelover",
    "NOTHELPFUL",
    "Dillygirl",
    "Leonrat",
    "KBMedia",
    "DocWilco",
    "Yingeling",
];

const COLOSSAL_FLF: &str = include_str!("fonts/colossal.flf");
const BASIC_FLF: &str = include_str!("fonts/basic.flf");

/// Cells per second for the message row — colossal glyphs read fine fast.
const SPEED: f64 = 60.0;

// ── Minimal figlet .flf loader ──────────────────────────────────────
// Full-width layout only (no smushing/kerning): every letter stands on
// its own. Header: `flf2a$ height baseline maxlen oldlayout comments`;
// the hardblank is the last char of the signature. Then 95 glyphs
// (ASCII 32–126), `height` lines each, trailing endmark chars stripped.

struct FigletFont {
    height: usize,
    glyphs: Vec<Vec<String>>, // index = ascii − 32
}

impl FigletFont {
    fn parse(src: &str) -> Self {
        let mut lines = src.lines();
        let header = lines.next().expect("empty .flf");
        let mut fields = header.split_whitespace();
        let sig = fields.next().expect(".flf signature");
        let hardblank = sig.chars().last().expect(".flf hardblank");
        let height: usize = fields.next().and_then(|f| f.parse().ok()).expect(".flf height");
        let _baseline = fields.next();
        let _max_len = fields.next();
        let _old_layout = fields.next();
        let comments: usize = fields.next().and_then(|f| f.parse().ok()).expect(".flf comments");
        for _ in 0..comments {
            lines.next();
        }

        let mut glyphs = Vec::with_capacity(95);
        'glyphs: while glyphs.len() < 95 {
            let mut rows = Vec::with_capacity(height);
            for _ in 0..height {
                let Some(raw) = lines.next() else { break 'glyphs };
                let line = raw.trim_end_matches('\r');
                let endmark = line.chars().last().unwrap_or('@');
                rows.push(line.trim_end_matches(endmark).replace(hardblank, " "));
            }
            glyphs.push(rows);
        }
        assert_eq!(glyphs.len(), 95, ".flf ended before ASCII 32-126 glyphs");
        Self { height, glyphs }
    }

    fn glyph(&self, ch: char) -> &Vec<String> {
        let idx = (ch as usize)
            .checked_sub(32)
            .filter(|&i| i < 95)
            .unwrap_or(0); // unknown chars render as space
        &self.glyphs[idx]
    }

    fn glyph_width(glyph: &[String]) -> usize {
        glyph.iter().map(|r| r.chars().count()).max().unwrap_or(0)
    }

    /// Render `text` into `height` rows, full-width layout.
    fn render(&self, text: &str) -> Vec<String> {
        let mut rows = vec![String::new(); self.height];
        for ch in text.chars() {
            let glyph = self.glyph(ch);
            let w = Self::glyph_width(glyph);
            for (i, row) in rows.iter_mut().enumerate() {
                row.push_str(&glyph[i]);
                for _ in glyph[i].chars().count()..w {
                    row.push(' ');
                }
            }
        }
        rows
    }

    /// Rendered width of `text` in cells.
    fn width(&self, text: &str) -> usize {
        text.chars().map(|ch| Self::glyph_width(self.glyph(ch))).sum()
    }
}

// ── Cue parsing ─────────────────────────────────────────────────────

/// Strip `{cueN}` markers from `msg`; return the display text and each
/// cue's `(n, char_index_in_display_text)`.
fn parse_cues(msg: &str) -> (String, Vec<(u32, usize)>) {
    let mut display = String::with_capacity(msg.len());
    let mut cues = Vec::new();
    let mut rest = msg;
    while let Some(start) = rest.find("{cue") {
        display.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let Some(end) = after.find('}') else {
            display.push_str(&rest[start..]);
            return (display, cues);
        };
        if let Ok(n) = after[..end].parse::<u32>() {
            cues.push((n, display.chars().count()));
        }
        rest = &after[end + 1..];
    }
    display.push_str(rest);
    (display, cues)
}

// ── Outline ─────────────────────────────────────────────────────────

// ── Layering ────────────────────────────────────────────────────────

/// Z-composite with a translucent outline. The names render first (back
/// layer); the message block composites over them. Its glyphs are
/// opaque; every empty message cell touching a glyph becomes a
/// 50%-alpha veil in a neutral stroke color:
///
/// - over a name showing through, the name's glyph is kept and its
///   color pulled halfway toward the stroke — dimmed, not blocked;
/// - over bare background, a `▒` half-shade block (50% of the cell's
///   pixels lit) in the stroke color.
struct Layered {
    back: NamesSequence,
    front: Box<dyn Effect>,
    front_h: usize,
    height: usize,
    /// Neutral outline color: terminal fg blended 50% toward bg.
    stroke: Color,
}

impl Effect for Layered {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.back.render(buf, frame);

        let mut front = FrameBuffer::new(buf.width, self.front_h);
        self.front.render(&mut front, frame);

        // Outline mask: empty front cells with a glyph in the 8-neighborhood.
        let mut outline = vec![false; buf.width * self.front_h];
        for y in 0..self.front_h {
            for x in 0..buf.width {
                if front.get(x, y).ch != ' ' {
                    continue;
                }
                'scan: for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0
                            || ny < 0
                            || nx as usize >= buf.width
                            || ny as usize >= self.front_h
                        {
                            continue;
                        }
                        if front.get(nx as usize, ny as usize).ch != ' ' {
                            outline[y * buf.width + x] = true;
                            break 'scan;
                        }
                    }
                }
            }
        }

        for y in 0..self.front_h.min(buf.height) {
            for x in 0..buf.width {
                let cell = front.get(x, y);
                if cell.ch != ' ' {
                    buf.set(x, y, cell); // opaque glyph
                } else if outline[y * buf.width + x] {
                    let behind = buf.get(x, y);
                    if behind.ch != ' ' {
                        // Name under the veil: dim it, don't block it.
                        buf.set_color(x, y, Color::lerp_rgb(behind.color, self.stroke, 0.5));
                    } else {
                        // Veil over bare background.
                        buf.set(x, y, Cell::new('▒', self.stroke));
                    }
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (0, self.height)
    }
}

// ── Name choreography ───────────────────────────────────────────────
// Per name: descend from above the top edge (BounceOut, with a sine
// wobble that fades as it lands) → dwell at the bottom → small hop →
// accelerate down off-screen. The next name starts at D/2.

struct NamesSequence {
    /// (rows, width, color) per name, basic font.
    names: Vec<(Vec<String>, usize, Color)>,
    name_h: usize,
    start_frame: usize,
    /// Frames between successive name starts (= total_d / 2).
    step: usize,
    d_desc: usize,
    d_dwell: usize,
    d_hop: usize,
    d_exit: usize,
}

impl NamesSequence {
    fn total_d(&self) -> usize {
        self.d_desc + self.d_dwell + self.d_hop + self.d_exit
    }
}

impl Effect for NamesSequence {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let d = self.total_d();
        for (i, (rows, w, color)) in self.names.iter().enumerate() {
            let start = self.start_frame + i * self.step;
            if frame < start {
                continue;
            }
            let local = frame - start;
            if local >= d {
                continue;
            }

            let h = self.name_h as f64;
            let y_land = buf.height.saturating_sub(self.name_h) as f64;
            let (base_y, wobble) = if local < self.d_desc {
                let u = local as f64 / self.d_desc as f64;
                let e = Easing::BounceOut.apply(u);
                (-h + (y_land + h) * e, 1.0 - e)
            } else if local < self.d_desc + self.d_dwell {
                (y_land, 0.0)
            } else if local < self.d_desc + self.d_dwell + self.d_hop {
                let u = (local - self.d_desc - self.d_dwell) as f64 / self.d_hop as f64;
                (y_land - 2.0 * (std::f64::consts::PI * u).sin(), 0.0)
            } else {
                let u = (local - self.d_desc - self.d_dwell - self.d_hop) as f64
                    / self.d_exit as f64;
                (y_land + Easing::EaseIn.apply(u) * (h + 2.0), 0.0)
            };

            let x0 = buf.width.saturating_sub(w + 2);
            for (ry, row) in rows.iter().enumerate() {
                for (rx, ch) in row.chars().enumerate() {
                    if ch == ' ' {
                        continue;
                    }
                    let x = x0 + rx;
                    if x >= buf.width {
                        continue;
                    }
                    let dy = ((x as f64 * 0.25 + tempo(frame) * 0.2).sin() * wobble).round();
                    let y = base_y + ry as f64 + dy;
                    if y < 0.0 || y as usize >= buf.height {
                        continue;
                    }
                    buf.set(x, y as usize, Cell::new(ch, *color));
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let colossal = FigletFont::parse(COLOSSAL_FLF);
    let basic = FigletFont::parse(BASIC_FLF);

    let (display, cues) = parse_cues(MESSAGE);
    let cue1_idx = cues
        .iter()
        .find(|(n, _)| *n == 1)
        .map(|&(_, i)| i)
        .expect("MESSAGE must contain a {cue1} marker");

    let term_w = terminal_width();
    let prefix: String = display.chars().take(cue1_idx).collect();
    let cue_col = colossal.width(&prefix);
    let cue1_secs = (cue_col as f64 - term_w as f64 / 2.0).max(0.0) / SPEED;

    // Message block: 4 pad rows, 11 colossal rows, 4 pad rows — headroom
    // for the ±3.5-cell wave; every row padded past the art so the
    // horizontal loop has a breather.
    let msg_rows = colossal.render(&display);
    let loop_w = colossal.width(&display) + 60;
    let mut block: Vec<String> = Vec::with_capacity(msg_rows.len() + 8);
    for _ in 0..4 {
        block.push(String::new());
    }
    block.extend(msg_rows.iter().map(|r| format!("{r:<loop_w$}")));
    for _ in 0..4 {
        block.push(String::new());
    }
    let message_block = block.join("\n");

    // Plasma flowing through the colossal glyphs — hot magenta/gold palette.
    let lava = gradient(&["#2b0a3d", "#ff3670", "#ffd166"]).palette(64);
    let message = VSineWave::wrap(
        Marquee::wrap(Plasma::on(&message_block).palette(lava).seed(7.0)).speed(SPEED),
    )
    .amplitude(3.5) // big bouncy swing (the block has 4 pad rows each side)
    .frequency(0.12) // ~52-col wavelength — the wave curls inside a word
    .speed(0.2) // crests at 50 cells/s — funky, well under the 90 ceiling
    .delay(8.0) // flat horizontal scroll first...
    .ramp(2.0); // ...then the wave fades in

    // Names: basic font, cycling accent colors.
    let palette = [
        Color::new(255, 211, 105),
        Color::new(112, 214, 255),
        Color::new(255, 138, 216),
        Color::new(154, 255, 154),
        Color::new(255, 156, 110),
    ];
    let names: Vec<(Vec<String>, usize, Color)> = NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| (basic.render(n), basic.width(n), palette[i % palette.len()]))
        .collect();

    let seq = NamesSequence {
        names,
        name_h: basic.height,
        start_frame: secs_to_frames(cue1_secs),
        step: secs_to_frames(2.4), // D/2
        d_desc: secs_to_frames(2.2),
        d_dwell: secs_to_frames(1.2),
        d_hop: secs_to_frames(0.6),
        d_exit: secs_to_frames(0.8),
    };
    let names_end = cue1_secs + (NAMES.len() as f64 - 1.0) * 2.4 + 4.8 + 1.0;
    let message_end = loop_w as f64 / SPEED + 2.0;
    let total = message_end.max(names_end);

    // Stage: names in back, 19-row message block in front, 4 landing rows
    // below it — 23 rows, plus the header line above.
    let stage = Layered {
        back: seq,
        front: Box::new(message),
        front_h: 19,
        height: 23,
        stroke: Color::lerp_rgb(bg_color(), fg_color(), 0.5),
    };

    Scene::new()
        .add(text("greetings — colossal edition", Color::new(110, 110, 110)))
        .add(stage)
        .quit_key('q')
        .run(total)
        .await;
}
