//! Parallax demo — the wooded landscape from `examples/parallax.rs` with
//! an extra layer on top: `chromakopia` slides in across the sky after
//! a brief beat, like a movie title appearing over the establishing shot.
//!
//! Run: `cargo run --release --example letters_parallax`

use chromakopia::animate::framebuffer::{secs_to_frames, tempo, Cell as RawCell, FrameBuffer as RawFB};
use chromakopia::prelude::*;

// ── XBin loader (icy_draw "eXtended Binary" format) ─────────────
// Standalone, zero-dependency parser. Supports:
//   - Header parse (XBIN\x1A magic, width/height, font-h, flags)
//   - Optional 48-byte palette (16 colours × 3 channels @ 0-63)
//   - Optional embedded font (skipped — we use the terminal's font
//     and rely on CP437→Unicode mapping for the glyph)
//   - Optional RLE compression (4 modes: raw, char-run, attr-run,
//     pair-run)
//   - iCE / non-blink mode (full 16-colour background)

const CGA_PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),       (0, 0, 170),     (0, 170, 0),     (0, 170, 170),
    (170, 0, 0),     (170, 0, 170),   (170, 85, 0),    (170, 170, 170),
    (85, 85, 85),    (85, 85, 255),   (85, 255, 85),   (85, 255, 255),
    (255, 85, 85),   (255, 85, 255),  (255, 255, 85),  (255, 255, 255),
];

#[rustfmt::skip]
const CP437: [char; 256] = [
    '\0','☺','☻','♥','♦','♣','♠','•','◘','○','◙','♂','♀','♪','♫','☼',
    '►','◄','↕','‼','¶','§','▬','↨','↑','↓','→','←','∟','↔','▲','▼',
    ' ','!','"','#','$','%','&','\'','(',')','*','+',',','-','.','/',
    '0','1','2','3','4','5','6','7','8','9',':',';','<','=','>','?',
    '@','A','B','C','D','E','F','G','H','I','J','K','L','M','N','O',
    'P','Q','R','S','T','U','V','W','X','Y','Z','[','\\',']','^','_',
    '`','a','b','c','d','e','f','g','h','i','j','k','l','m','n','o',
    'p','q','r','s','t','u','v','w','x','y','z','{','|','}','~','⌂',
    'Ç','ü','é','â','ä','à','å','ç','ê','ë','è','ï','î','ì','Ä','Å',
    'É','æ','Æ','ô','ö','ò','û','ù','ÿ','Ö','Ü','¢','£','¥','₧','ƒ',
    'á','í','ó','ú','ñ','Ñ','ª','º','¿','⌐','¬','½','¼','¡','«','»',
    '░','▒','▓','│','┤','╡','╢','╖','╕','╣','║','╗','╝','╜','╛','┐',
    '└','┴','┬','├','─','┼','╞','╟','╚','╔','╩','╦','╠','═','╬','╧',
    '╨','╤','╥','╙','╘','╒','╓','╫','╪','┘','┌','█','▄','▌','▐','▀',
    'α','ß','Γ','π','Σ','σ','µ','τ','Φ','Θ','Ω','δ','∞','φ','ε','∩',
    '≡','±','≥','≤','⌠','⌡','÷','≈','°','∙','·','√','ⁿ','²','■',' ',
];

/// XBin art loaded into a flat cell grid.
struct XBinArt {
    width: usize,
    height: usize,
    palette: [(u8, u8, u8); 16],
    /// One (char, fg_idx, bg_idx) per cell, row-major.
    cells: Vec<(char, u8, u8)>,
}

impl XBinArt {
    fn load(path: &str) -> Self {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("couldn't read {path}: {e}"));
        Self::parse(&bytes).unwrap_or_else(|e| panic!("invalid XBin {path}: {e}"))
    }

    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 11 || &bytes[..5] != b"XBIN\x1A" {
            return Err("missing XBIN\\x1A magic".into());
        }
        let w = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let h = u16::from_le_bytes([bytes[7], bytes[8]]) as usize;
        let font_h = bytes[9] as usize;
        let flags = bytes[10];
        let has_palette = flags & 0x01 != 0;
        let has_font = flags & 0x02 != 0;
        let compressed = flags & 0x04 != 0;
        let non_blink = flags & 0x08 != 0;
        let ext_chars = flags & 0x10 != 0;

        let mut p = 11;

        let palette = if has_palette {
            if bytes.len() < p + 48 { return Err("palette truncated".into()); }
            let mut pal = CGA_PALETTE;
            for i in 0..16 {
                let scale = |v: u8| (v as u32 * 255 / 63) as u8;
                pal[i] = (
                    scale(bytes[p + i * 3]),
                    scale(bytes[p + i * 3 + 1]),
                    scale(bytes[p + i * 3 + 2]),
                );
            }
            p += 48;
            pal
        } else {
            CGA_PALETTE
        };

        if has_font {
            let n = if ext_chars { 512 } else { 256 };
            p += font_h * n;
        }

        let total = w * h;
        let mut cells = Vec::with_capacity(total);

        let make = |ch: u8, at: u8| -> (char, u8, u8) {
            let glyph = CP437[ch as usize];
            let fg = at & 0x0F;
            let bg = if non_blink { (at >> 4) & 0x0F } else { (at >> 4) & 0x07 };
            (glyph, fg, bg)
        };

        if compressed {
            while cells.len() < total {
                if p >= bytes.len() { return Err("RLE truncated".into()); }
                let hdr = bytes[p]; p += 1;
                let count = (hdr & 0x3F) as usize + 1;
                let mode = (hdr >> 6) & 0x03;
                match mode {
                    0 => for _ in 0..count {
                        if p + 1 >= bytes.len() { return Err("raw block truncated".into()); }
                        cells.push(make(bytes[p], bytes[p + 1]));
                        p += 2;
                    },
                    1 => {
                        if p >= bytes.len() { return Err("char-run truncated".into()); }
                        let ch = bytes[p]; p += 1;
                        for _ in 0..count {
                            if p >= bytes.len() { return Err("char-run attrs truncated".into()); }
                            cells.push(make(ch, bytes[p]));
                            p += 1;
                        }
                    },
                    2 => {
                        if p >= bytes.len() { return Err("attr-run truncated".into()); }
                        let at = bytes[p]; p += 1;
                        for _ in 0..count {
                            if p >= bytes.len() { return Err("attr-run chars truncated".into()); }
                            cells.push(make(bytes[p], at));
                            p += 1;
                        }
                    },
                    3 => {
                        if p + 1 >= bytes.len() { return Err("pair-run truncated".into()); }
                        let ch = bytes[p]; let at = bytes[p + 1]; p += 2;
                        for _ in 0..count { cells.push(make(ch, at)); }
                    },
                    _ => unreachable!(),
                }
            }
        } else {
            if bytes.len() < p + total * 2 { return Err("raw image truncated".into()); }
            for i in 0..total {
                cells.push(make(bytes[p + i * 2], bytes[p + i * 2 + 1]));
            }
        }

        cells.truncate(total);
        Ok(Self { width: w, height: h, palette, cells })
    }
}

impl Effect for XBinArt {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for y in 0..self.height.min(buf.height) {
            for x in 0..self.width.min(buf.width) {
                let (ch, fg_i, bg_i) = self.cells[y * self.width + x];
                // Treat space-on-black as transparent so the sky shows
                // through wherever the artist left the canvas empty.
                if ch == ' ' && bg_i == 0 { continue; }
                let (fr, fg, fb) = self.palette[fg_i as usize];
                let (br, bg, bb) = self.palette[bg_i as usize];
                buf.set(x, y, RawCell::with_bg(
                    ch,
                    Color::new(fr, fg, fb),
                    Color::new(br, bg, bb),
                ));
            }
        }
    }
    fn size(&self) -> (usize, usize) { (self.width, self.height) }
}

/// Opaque-black base used during the intro so stars fade in against
/// true black before any other layer slides into view. Paints across
/// the full buffer width (matches WinterGlow's pattern), so the
/// black bg covers the entire scene regardless of terminal width.
struct BlackBg {
    height: usize,
}
impl Effect for BlackBg {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        let black = Color::new(0, 0, 0);
        for y in 0..buf.height.min(self.height) {
            for x in 0..buf.width {
                buf.set(x, y, RawCell::with_bg('█', black, black));
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (80, self.height)
    }
}

/// Render an inner effect that is taller than the output buffer, and
/// animate a viewport-sized window down through it.
///
/// The viewport position holds at `initial_y` for `delay_secs`, then
/// eases (EaseInOut) from `initial_y` to `final_y` over `duration_secs`.
/// The inner effect always sees the **global** frame untouched, so any
/// time-evolving effect inside (e.g. the stars' Plasma twinkle) keeps
/// running during the hold. To freeze specific layers (like marquee
/// scroll) during the hold, wrap those layers individually.
struct PanDown<E: Effect> {
    inner: E,
    content_h: usize,
    delay_frames: usize,
    duration_frames: usize,
    initial_y: i32,
    final_y: i32,
}
impl<E: Effect> PanDown<E> {
    fn new(
        inner: E,
        content_h: usize,
        delay_secs: f64,
        duration_secs: f64,
        initial_y: i32,
        final_y: i32,
    ) -> Self {
        Self {
            inner,
            content_h,
            delay_frames: secs_to_frames(delay_secs),
            duration_frames: secs_to_frames(duration_secs).max(1),
            initial_y,
            final_y,
        }
    }
}
impl<E: Effect> Effect for PanDown<E> {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let view_y = if frame < self.delay_frames {
            self.initial_y
        } else {
            let local = frame - self.delay_frames;
            let t = (local as f64 / self.duration_frames as f64).min(1.0);
            let eased = Easing::EaseInOut.apply(t);
            (self.initial_y as f64 + (self.final_y - self.initial_y) as f64 * eased).round() as i32
        };

        let mut scratch = RawFB::new(buf.width.max(1), self.content_h.max(buf.height));
        self.inner.render(&mut scratch, frame);

        for dy in 0..buf.height {
            let sy = view_y + dy as i32;
            if sy < 0 || sy >= scratch.height as i32 { continue; }
            for x in 0..buf.width {
                let cell = scratch.get(x, sy as usize);
                if cell.ch == ' ' && cell.bg.is_none() { continue; }
                buf.set(x, dy, cell);
            }
        }
    }
    fn size(&self) -> (usize, usize) { self.inner.size() }
}

/// Custom marquee-like wrapper with a *split* frame counter:
/// - the inner effect is always called with the **global frame**, so
///   anything time-evolving (e.g. Plasma colour drift) keeps running
///   from t=0;
/// - the horizontal scroll offset is computed from `frame - delay`,
///   so the tile motion stays frozen at offset 0 during the delay
///   and then starts swinging from rest when the delay ends.
///
/// The swing maths mirror chromakopia's `Marquee.swing` so this layer's
/// parallax stays in phase with the other landscape marquees that
/// also begin their swing cycles at the same global moment.
struct ParallaxTile<E: Effect> {
    inner: E,
    content_w: usize,
    content_h: usize,
    speed: f64,
    cycle_secs: f64,
    delay_frames: usize,
}
impl<E: Effect> ParallaxTile<E> {
    fn new(
        inner: E,
        content_w: usize,
        content_h: usize,
        speed: f64,
        cycle_secs: f64,
        delay_secs: f64,
    ) -> Self {
        Self {
            inner,
            content_w,
            content_h,
            speed,
            cycle_secs,
            delay_frames: secs_to_frames(delay_secs),
        }
    }
}
impl<E: Effect> Effect for ParallaxTile<E> {
    fn render(&self, out: &mut FrameBuffer, frame: usize) {
        if self.content_w == 0 || self.content_h == 0 { return; }

        let mut scratch = RawFB::new(self.content_w, self.content_h);
        self.inner.render(&mut scratch, frame);

        let scroll_frame = frame.saturating_sub(self.delay_frames);
        let t = scroll_frame as f64 / chromakopia::animate::framebuffer::FPS;
        let offset_f = if self.cycle_secs > 0.0 {
            let amplitude = self.speed * self.cycle_secs * 0.25;
            let phase = std::f64::consts::TAU * t / self.cycle_secs;
            amplitude * phase.sin()
        } else {
            t * self.speed
        };
        let offset = offset_f.floor() as i64;

        for y in 0..out.height.min(self.content_h) {
            for x in 0..out.width {
                let src_x = (x as i64 + offset).rem_euclid(self.content_w as i64) as usize;
                let cell = scratch.get(src_x, y);
                if cell.ch != ' ' {
                    out.set(x, y, cell);
                }
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (self.content_w, self.content_h)
    }
}

/// Hold the inner effect's frame counter at 0 for `delay_secs`, then
/// pass through `frame - delay`. Time-evolving effects inside (Plasma
/// colour drift, Marquee scroll position, etc.) freeze during the
/// delay and resume from t=0 when the delay ends.
struct FreezeFrame<E: Effect> {
    inner: E,
    delay_frames: usize,
}
impl<E: Effect> FreezeFrame<E> {
    fn new(inner: E, delay_secs: f64) -> Self {
        Self { inner, delay_frames: secs_to_frames(delay_secs) }
    }
}
impl<E: Effect> Effect for FreezeFrame<E> {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let effective = if frame < self.delay_frames { 0 } else { frame - self.delay_frames };
        self.inner.render(buf, effective);
    }
    fn size(&self) -> (usize, usize) { self.inner.size() }
}

/// Title text that slides in from off-screen right and eases to centered.
/// Each lit cell picks a colour from `palette` via a low-amplitude plasma
/// field — subtle drift so the foliage greens shimmer like leaves rather
/// than reading as a flat block of colour.
struct SlideTitle {
    lines: Vec<Vec<char>>,
    text_w: usize,
    text_h: usize,
    duration_frames: usize,
    palette: Vec<Color>,
    bg: Color,
}

impl SlideTitle {
    fn new(text: &str, duration_secs: f64, palette: Vec<Color>, bg: Color) -> Self {
        let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let text_w = lines.iter().map(|l| l.len()).max().unwrap_or(0);
        let text_h = lines.len();
        Self {
            lines,
            text_w,
            text_h,
            duration_frames: secs_to_frames(duration_secs),
            palette,
            bg,
        }
    }
}

impl Effect for SlideTitle {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let buf_w = buf.width as i32;
        let target_x = (buf.width as i32 - self.text_w as i32) / 2;
        let start_x = buf_w;

        let t = if self.duration_frames == 0 {
            1.0
        } else {
            (frame as f64 / self.duration_frames as f64).min(1.0)
        };
        let eased = Easing::EaseOut.apply(t);
        let cur_x = start_x as f64 + (target_x as f64 - start_x as f64) * eased;
        let cur_x = cur_x.round() as i32;

        // Plasma-ish per-cell colour. Fast time-evolving sum of sines
        // maps each cell to a palette index — boiling leaf shimmer.
        let pal = &self.palette;
        let last = pal.len().saturating_sub(1) as f64;
        let pt = tempo(frame) * 0.08;

        for (ly, line) in self.lines.iter().enumerate() {
            if ly >= buf.height {
                break;
            }

            // 1) Fill bg across the line's span (first non-space to last
            //    non-space). This paints the title's own dark backdrop into
            //    the inter-glyph gaps so the scene doesn't bleed through.
            if let (Some(first), Some(last_x)) = (
                line.iter().position(|&c| c != ' '),
                line.iter().rposition(|&c| c != ' '),
            ) {
                for lx in first..=last_x {
                    let tx = cur_x + lx as i32;
                    if tx < 0 || tx >= buf_w {
                        continue;
                    }
                    buf.set(tx as usize, ly, RawCell::with_bg(' ', self.bg, self.bg));
                }
            }

            // 2) Write the glyphs over the bg, keeping bg = title bg.
            for (lx, &ch) in line.iter().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let tx = cur_x + lx as i32;
                if tx < 0 || tx >= buf_w {
                    continue;
                }
                let v = (lx as f64 * 0.13 + pt).sin()
                    + (ly as f64 * 0.22 + pt * 0.7).cos()
                    + ((lx as f64 + ly as f64) * 0.09 + pt * 0.5).sin();
                let norm = ((v + 3.0) / 6.0).clamp(0.0, 1.0);
                let idx = (norm * last).round() as usize;
                let color = pal[idx.min(pal.len() - 1)];
                buf.set(tx as usize, ly, RawCell::with_bg(ch, color, self.bg));
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (200, self.text_h)
    }
}

/// Winter sky glow — soft vertical gradient peaking near the horizon row
/// (where the mountains will sit). Cells render as full blocks with the
/// gradient colour as bg, so overlays composited on top (stars, etc.)
/// inherit the sky tint where they have no explicit bg.
struct WinterGlow {
    height: usize,
    horizon_row: usize,
    horizon_color: Color,
    deep_color: Color,
}
impl Effect for WinterGlow {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        let h = self.height.min(buf.height);
        for y in 0..h {
            let dist = (y as i32 - self.horizon_row as i32).abs() as f64;
            let max_dist = self.horizon_row.max(h.saturating_sub(self.horizon_row)) as f64;
            let t = (dist / max_dist.max(1.0)).clamp(0.0, 1.0);
            let t = t.powf(1.3); // quadratic-ish falloff — concentrate glow near horizon
            let color = Color::lerp_rgb(self.horizon_color, self.deep_color, t);
            for x in 0..buf.width {
                buf.set(x, y, RawCell::with_bg('█', color, color));
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (80, self.height)
    }
}

/// Forest-floor gradient. Fills its band with full-block cells; bg = fg
/// so overlays composited on top inherit this colour.
struct Ground {
    near: Color,
    far: Color,
    height: usize,
}
impl Effect for Ground {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        let h = self.height.min(buf.height);
        for y in 0..h {
            let t = if h > 1 {
                y as f64 / (h - 1) as f64
            } else {
                0.0
            };
            let color = Color::lerp_rgb(self.far, self.near, t);
            for x in 0..buf.width {
                buf.set(x, y, RawCell::with_bg('█', color, color));
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (80, self.height)
    }
}

/// Cloud silhouette lit from the right (sun/moon side). Each
/// contiguous non-space run on a row is colour-graded from `shadow`
/// on its left edge to `lit` on its right edge — same trick as
/// LitMountains, but no explicit `bg` so the half-block glyphs
/// (`▄`/`▀`) still show their half-cell silhouette against the sky
/// behind. Letting bg=fg would turn each cell into a full opaque
/// block and kill the fluffy edges.
struct LitClouds {
    art: Vec<Vec<char>>,
    shadow: Color,
    lit: Color,
}
impl LitClouds {
    fn new(text: &str, shadow: Color, lit: Color) -> Self {
        let art: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        Self { art, shadow, lit }
    }
}
impl Effect for LitClouds {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for (y, line) in self.art.iter().enumerate() {
            if y >= buf.height { break; }
            let mut x = 0usize;
            while x < line.len() && x < buf.width {
                if line[x] == ' ' { x += 1; continue; }
                let mut end = x;
                while end < line.len() && end < buf.width && line[end] != ' ' { end += 1; }
                let run_len = (end - x).max(1) as f64;
                for cx in x..end {
                    let t = (cx - x) as f64 / run_len;
                    let color = Color::lerp_rgb(self.shadow, self.lit, t);
                    buf.set(cx, y, RawCell::new(line[cx], color));
                }
                x = end;
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (
            self.art.iter().map(|l| l.len()).max().unwrap_or(0),
            self.art.len(),
        )
    }
}

/// Mountain silhouette lit from the right (where the implied moon hangs).
struct LitMountains {
    art: Vec<Vec<char>>,
    shadow: Color,
    lit: Color,
}
impl LitMountains {
    fn new(text: &str, shadow: Color, lit: Color) -> Self {
        let art: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        Self { art, shadow, lit }
    }
}
impl Effect for LitMountains {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for (y, line) in self.art.iter().enumerate() {
            if y >= buf.height {
                break;
            }
            let mut x = 0usize;
            while x < line.len() && x < buf.width {
                if line[x] == ' ' {
                    x += 1;
                    continue;
                }
                let mut end = x;
                while end < line.len() && end < buf.width && line[end] != ' ' {
                    end += 1;
                }
                let run_len = (end - x).max(1) as f64;
                for cx in x..end {
                    let t = (cx - x) as f64 / run_len;
                    let color = Color::lerp_rgb(self.shadow, self.lit, t);
                    buf.set(cx, y, RawCell::with_bg(line[cx], color, color));
                }
                x = end;
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (
            self.art.iter().map(|l| l.len()).max().unwrap_or(0),
            self.art.len(),
        )
    }
}

/// Multi-glyph tree art with row-keyed background:
/// - `█` solid block canopy   (bg = crown — fully opaque)
/// - `▓` solid block trunk    (bg = trunk — fully opaque)
/// - `▲` narrow triangle canopy (bg = `bg_per_row[y]` — punches sky/ground
///       through the mid layer behind so triangles don't show mid-green
///       bleed-through in the inter-glyph gaps)
/// - `┃` narrow green stem    (bg = `bg_per_row[y]` — same trick)
///
/// `bg_per_row` is precomputed at construction (sky gradient for upper
/// rows, ground gradient for the bottom 2 rows), so per-cell cost is
/// just a `Vec` lookup.
struct ColoredTrees {
    art: Vec<Vec<char>>,
    crown: Color,
    trunk: Color,
    stem: Color,
    bg_per_row: Vec<Color>,
}
impl ColoredTrees {
    fn new(text: &str, crown: Color, trunk: Color, stem: Color, bg_per_row: Vec<Color>) -> Self {
        let art = text.lines().map(|l| l.chars().collect()).collect();
        Self {
            art,
            crown,
            trunk,
            stem,
            bg_per_row,
        }
    }
}
impl Effect for ColoredTrees {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for (y, line) in self.art.iter().enumerate() {
            if y >= buf.height {
                break;
            }
            let row_bg = self
                .bg_per_row
                .get(y)
                .copied()
                .unwrap_or(Color::new(0, 0, 0));
            for (x, &ch) in line.iter().enumerate() {
                if x >= buf.width {
                    break;
                }
                let cell = match ch {
                    '█' => RawCell::new('█', self.crown),
                    '▓' => RawCell::new('█', self.trunk),
                    '▲' => RawCell::with_bg('▲', self.crown, row_bg),
                    '┃' => RawCell::with_bg('┃', self.stem, row_bg),
                    _ => continue,
                };
                buf.set(x, y, cell);
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (
            self.art.iter().map(|l| l.len()).max().unwrap_or(0),
            self.art.len(),
        )
    }
}

// 40 rows of starfield. The bottom 22 rows are the "regular horizon"
// view that fills the scene after the intro tilt. The top 18 rows are
// extra "high-altitude" sky — denser/varied — that the camera pans
// down through during phase 2 to simulate looking up at the night sky
// and then tilting forward.
const STARS: &str = "
   *                   .                   *                .                   *
              .                   *                   .              *
       *                                  *                    .             *
   .                  *                       .                  *                  *
                  .                  *                .                  *
       *                            .                 *                       .
   .                       *                                .             *           *
              *                            .                  *
       .                  *                   .                  *               .
   *                                .                 *                  *
                  *                      .                 *                  .
       .                            *                            .              *
              .                   *                    .                 *
   *                  .                  *                   .                       *
       .                            *                                  *
              *                  .                  *                  .
       .                  *                .                   *                 .
       .                            *                   .                 *
                  *                          .                  *
   .                       *                          .                    .
                .                       *                    *
       *                  .             *               .                  *
   *                  .              *                    .
            .                  *                .                       *
   .              *                       .         *
                       .             *                       .            *
       *                  .                  *                   .
                  .              *                  .             *
            *                 .                  *                  .
   .              *                  .                  *
                       *                  .                 *                  .
         .                  *                .                   *
   *                  .                  *                   .             *
            .                  *                  .                  *
                  *                  .                  *                  .
       .                  *                .                   *
   .              *                 .                  *                  .
                       *                  .                 *
   *                  .              *                  .";

// ── Cloud layers ───────────────────────────────────────────────
// Four bands of half-block puffs at progressively larger sizes.
// Each marquee scrolls at a different speed (furthest = slowest),
// so the resulting parallax fans the sky horizontally between
// the static stars and the mountain silhouette.

// Each cloud uses `▄` for its top edge (cell's lower half), `▒` for
// the body (50/50 dither — combined with the inherited sky bg it
// reads as a semi-transparent puff) and `▀` for the bottom edge.

const CLOUDS_1: &str = "
       ▄▄▄▄▄▄                          ▄▄▄▄▄▄▄▄                              ▄▄▄▄▄▄
      ▄▒▒▒▒▒▒▒▒▄                      ▄▒▒▒▒▒▒▒▒▒▒▄                          ▄▒▒▒▒▒▒▒▒▄
     ▒▒▒▒▒▒▒▒▒▒▒▒                    ▒▒▒▒▒▒▒▒▒▒▒▒▒▒                        ▒▒▒▒▒▒▒▒▒▒▒▒
      ▒▒▒▒▒▒▒▒▒▒                      ▒▒▒▒▒▒▒▒▒▒▒▒                          ▒▒▒▒▒▒▒▒▒▒
       ▀▀▀▀▀▀▀▀                        ▀▀▀▀▀▀▀▀▀▀                            ▀▀▀▀▀▀▀▀";

const CLOUDS_2: &str = "
          ▄▄▄▄▄▄▄▄▄                              ▄▄▄▄▄▄▄▄▄▄▄                              ▄▄▄▄▄▄▄▄▄
         ▄▒▒▒▒▒▒▒▒▒▒▒▄                          ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▄                          ▄▒▒▒▒▒▒▒▒▒▒▒▄
        ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                     ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
         ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                         ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
          ▀▀▀▀▀▀▀▀▀▀▀▀▀                          ▀▀▀▀▀▀▀▀▀▀▀▀▀▀                          ▀▀▀▀▀▀▀▀▀▀▀▀▀";

const CLOUDS_3: &str = "
            ▄▄▄▄▄▄▄▄▄▄▄▄                                ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄                          ▄▄▄▄▄▄▄▄▄▄▄▄
           ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄                            ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄                      ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄
          ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                      ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
          ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                      ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
           ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                        ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
            ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                          ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                    ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀";

const CLOUDS_4: &str = "
              ▄▄▄▄▄▄▄▄▄▄▄▄▄▄                                  ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄                              ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
             ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄                              ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄                        ▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄
            ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
            ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                       ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
             ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                         ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                    ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
              ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                           ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                      ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒
               ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                              ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                          ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀";

const FAR_MOUNTAINS: &str = "
       █                      █                         █                       █
      ███                    ███                       ███                     ███
     █████        █         █████          █         █████        █          █████
    ███████      ███       ███████        ███       ███████      ███        ███████
   █████████    █████     █████████      █████     █████████    █████      █████████
  ███████████  ███████   ███████████    ███████   ███████████  ███████    ███████████
 █████████████████████████████████████████████████████████████████████████████████████
███████████████████████████████████████████████████████████████████████████████████████
███████████████████████████████████████████████████████████████████████████████████████";

const MOUNTAINS: &str = "
                █                                                        █
               ███                █                                     ███
              █████              ███           █                       █████
         ██  ████████        █  █████         ███   █       █         ███████
        ████████████████  ████████████████  ███████ ███   █████   ███████████████
   ██████████████████████████████████████████████████████████████████████████████
████████████████████████████████████████████████████████████████████████████████";

// Trees rendered as pure `█` silhouettes — no half-blocks. Inside
// each layer the silhouettes vary in height (2–7 rows), thickness
// (3–13 cells wide), and top position (some tall trees clip against
// the top of the layer; some short shrubs only occupy the bottom row
// or two). The forest density stays high but the canopy edge now
// reads as irregular terrain instead of a row of clones.

// Front treeline: 8 rows of tiered spruce silhouettes — `█` canopy,
// `▓` trunk. Species mix: grand pines (11-wide solid pyramids, 3-wide
// trunks), towering spruces (pinched tiers), tiered firs, slim
// cypress columns, half-height firs, and one gnarled dead snag.
const TREES: &str = "
      █           █                  █        █         █                 █
     ███         ███                ███      ███       ███               ███
    █████       █████              █████     ███      █████             █████
   ███████       ███       █        ███      ███     ███████      ▓      ███
  █████████     █████     ███      █████     ███    █████████    ▓▓     █████
 ███████████   ███████   █████    ███████    ███   ███████████    ▓    ███████
     ▓▓▓         ▓▓        ▓         ▓        ▓        ▓▓▓        ▓      ▓▓
     ▓▓▓         ▓▓        ▓         ▓        ▓        ▓▓▓        ▓      ▓▓";

const MID_TREES: &str = "
  █       █           █            █       █           █            █       █
 ███     ███         ███          ███     ███         ███          ███     ███
  █     █████        ███    █      █     █████        ███    █      █     █████
 ███   ███████       ███   ███    ███   ███████       ███   ███    ███   ███████
█████     █      █    █   █████  █████     █      █    █   █████  █████     █
  █       █     ███   █     █      █       █     ███   █     █      █       █";

const FAR_MID_TREES: &str = "
  █          █       █ █    █          █       █ █    █   █               █   █
 ███      █  █      █████  ███   █     █      █████  ███  █   █          ███  █
█████    ███ █  ███  █ █  █████ ███    █  ███  █ █  █████ █  ███     ████████ █
  █   █   █  █   █   █ █    █    █  █  █   █   █ █    █   █   █   █   █   █   █";

const DISTANT_TREES: &str = "
 █    █ █  █     █     █    █      █     █    █  █ █     █     █    █  █ █    █
███  ████████ █ ███   ███  ███   ████   ███  ████████  ████   ███  ████████  ███
 █  █ █ █  █  █  █   █ █    █    █ █  █  █    █  █ █   █ █  █  █    █  █ █  █ █";

// figlet -f rozzo "chromakopia" — title that slides in over the scene
const CHROMAKOPIA: &str = "
         888                                          888                       ,e,
 e88'888 888 ee  888,8,  e88 88e  888 888 8e   ,\"Y88b 888 ee  e88 88e  888 88e   \"   ,\"Y88b
d888  '8 888 88b 888 \"  d888 888b 888 888 88b \"8\" 888 888 P  d888 888b 888 888b 888 \"8\" 888
Y888   , 888 888 888    Y888 888P 888 888 888 ,ee 888 888 b  Y888 888P 888 888P 888 ,ee 888
 \"88,e8' 888 888 888     \"88 88\"  888 888 888 \"88 888 888 8b  \"88 88\"  888 88\"  888 \"88 888
                                                                       888
                                                                       888";

#[tokio::main]
async fn main() {
    let cycle_secs = 12.0;
    let easing = Easing::EaseInOut;
    let base_speed = 3.0;

    // Viewport = what the terminal shows. Scene = the assembled
    // content, which is taller than the viewport so we can scroll
    // a window down it during the intro. Stars fill the full scene
    // height; landscape is anchored to the bottom.
    let viewport_h: usize = 22;
    let scene_h: usize = viewport_h * 2; // 44 rows
    let horizon_row: usize = scene_h - 4; // 4 rows above scene bottom
    let sky_horizon = Color::new(70, 92, 130);
    let sky_deep = Color::new(6, 10, 20);
    let ground_near = Color::new(88, 60, 32);
    let ground_far = Color::new(36, 24, 14);
    let ground_h: usize = 3;
    let trees_offset_from_bottom: usize = 8;
    let trees_h: usize = 8;

    // ── Layers ──────────────────────────────────────────────────
    let hold_secs: f64 = 3.0;

    let star_palette = gradient(&["#9ab5ff", "#ffffff", "#fff3c8", "#ffd28a"]).palette(8);
    // Stars: ParallaxTile so the Plasma inside always sees the real
    // frame (twinkle from t=0) but the horizontal swing offset stays
    // frozen for `hold_secs` and then begins from rest, in phase with
    // every other landscape marquee (which freezes via FreezeFrame
    // for the same duration).
    let stars = ParallaxTile::new(
        Plasma::on(STARS).palette(star_palette).seed(7.0),
        80,
        40, // STARS const has 40 lines of starfield
        0.0,           // sky doesn't parallax — only twinkles
        cycle_secs,
        hold_secs,
    );

    let winter_glow = WinterGlow {
        height: scene_h,
        horizon_row,
        horizon_color: sky_horizon,
        deep_color: sky_deep,
    };

    // ── Four cloud bands. Each is lit from the right (shadow on the
    //    left of every puff, lit on the right) via LitClouds — the
    //    same trick as the mountains. Atmospheric perspective: far
    //    bands are dimmer and more blue, near bands brighter and
    //    closer to pale moonlight white. All FreezeFrame-wrapped so
    //    they stay still during the static-sky hold and then start
    //    swinging together at hold-end.
    let make_cloud = |art: &'static str, shadow: Color, lit: Color, speed_mul: f64, h: usize| {
        FreezeFrame::new(
            Marquee::wrap(LitClouds::new(art, shadow, lit))
                .speed(base_speed * speed_mul)
                .direction(ScrollDirection::Left)
                .cycle(cycle_secs, easing)
                .swing()
                .size(120, h), // wide content so 80-col buf doesn't crop the big clouds
            hold_secs,
        )
    };
    let clouds_1 = make_cloud(
        CLOUDS_1,
        Color::new(35, 45, 60), Color::new(75, 85, 100),
        0.3, 5,
    );
    let clouds_2 = make_cloud(
        CLOUDS_2,
        Color::new(55, 65, 85), Color::new(110, 120, 135),
        0.6, 5,
    );
    let clouds_3 = make_cloud(
        CLOUDS_3,
        Color::new(75, 85, 105), Color::new(150, 160, 175),
        1.0, 6,
    );
    let clouds_4 = make_cloud(
        CLOUDS_4,
        Color::new(95, 105, 125), Color::new(190, 195, 205),
        1.5, 7,
    );

    // XBin clouds — two layers of icy_draw artwork. Each is 80×25
    // cells, rendered via CP437→Unicode and the CGA palette. cloud-2
    // drifts slower (further away in the depth hierarchy), cloud-1
    // drifts faster (closer to the camera, in front of cloud-2).
    let xbin_cloud_2 = FreezeFrame::new(
        Marquee::wrap(XBinArt::load("ans/cloud-2.xb"))
            .speed(base_speed * 1.1)
            .direction(ScrollDirection::Left)
            .cycle(cycle_secs, easing)
            .swing()
            .size(80, 25),
        hold_secs,
    );
    let xbin_cloud_1 = FreezeFrame::new(
        Marquee::wrap(XBinArt::load("ans/cloud-1.xb"))
            .speed(base_speed * 1.8)
            .direction(ScrollDirection::Left)
            .cycle(cycle_secs, easing)
            .swing()
            .size(80, 25),
        hold_secs,
    );

    let far_mountains = FreezeFrame::new(
        Marquee::wrap(LitMountains::new(
            FAR_MOUNTAINS,
            Color::new(28, 32, 48),
            Color::new(50, 58, 78),
        ))
        .speed(base_speed * 2.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 9),
        hold_secs,
    );

    let mountains = FreezeFrame::new(
        Marquee::wrap(LitMountains::new(
            MOUNTAINS,
            Color::new(48, 52, 76),
            Color::new(95, 102, 130),
        ))
        .speed(base_speed * 4.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 7),
        hold_secs,
    );

    let ground = Ground {
        near: ground_near,
        far: ground_far,
        height: ground_h,
    };

    let distant_trees = FreezeFrame::new(
        Marquee::wrap(Solid(Color::new(6, 16, 10)).on(DISTANT_TREES))
            .speed(base_speed * 5.0)
            .direction(ScrollDirection::Left)
            .cycle(cycle_secs, easing)
            .swing()
            .size(80, 3),
        hold_secs,
    );
    let far_mid_trees = FreezeFrame::new(
        Marquee::wrap(Solid(Color::new(8, 22, 12)).on(FAR_MID_TREES))
            .speed(base_speed * 6.0)
            .direction(ScrollDirection::Left)
            .cycle(cycle_secs, easing)
            .swing()
            .size(80, 4),
        hold_secs,
    );
    let mid_trees = FreezeFrame::new(
        Marquee::wrap(Solid(Color::new(11, 28, 15)).on(MID_TREES))
            .speed(base_speed * 7.0)
            .direction(ScrollDirection::Left)
            .cycle(cycle_secs, easing)
            .swing()
            .size(80, 6),
        hold_secs,
    );

    // Bg colour the front tree layer paints into transparent inter-glyph
    // cells. Upper rows: a muted dark blue close to mountain shadow.
    // Lower rows (overlapping the ground band): the ground gradient.
    let trees_top_scene_row = scene_h - trees_offset_from_bottom;
    let ground_top_scene_row = scene_h - ground_h;
    // Match the mid-forest tone (its fg is rgb(11,28,15)) so triangle
    // tree cells blend into the green silhouette behind them instead
    // of punching a sky-blue rectangle through the forest.
    let canopy_bg = Color::new(8, 20, 12);
    let trees_bg: Vec<Color> = (0..trees_h)
        .map(|y| {
            let scene_y = trees_top_scene_row + y;
            if scene_y >= ground_top_scene_row {
                let gy = scene_y - ground_top_scene_row;
                let t = if ground_h > 1 { gy as f64 / (ground_h - 1) as f64 } else { 0.0 };
                Color::lerp_rgb(ground_far, ground_near, t)
            } else {
                canopy_bg
            }
        })
        .collect();

    let trees = FreezeFrame::new(
        Marquee::wrap(ColoredTrees::new(
            TREES,
            Color::new(15, 38, 20),
            Color::new(30, 18, 10),
            Color::new(38, 78, 44),
            trees_bg,
        ))
        .speed(base_speed * 10.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, trees_h),
        hold_secs,
    );

    let title_palette = vec![
        Color::new(44, 74, 48),
        Color::new(58, 98, 64),
        Color::new(92, 145, 102),
        Color::new(126, 184, 136),
        Color::new(92, 145, 102),
        Color::new(58, 98, 64),
    ];
    // Title lands after the static-sky hold + the pan + a brief beat.
    let title = DelayedStart::new(
        hold_secs + 2.5,
        SlideTitle::new(
            CHROMAKOPIA.trim_matches('\n'),
            2.0,
            title_palette,
            Color::new(6, 12, 10),
        ),
    );

    // ── Inner scene (scene_h rows tall, taller than viewport) ──
    //    WinterGlow + stars span the full scene height; landscape is
    //    anchored to the scene's bottom. Title sits low enough that
    //    it lands in the middle-lower part of the viewport's final
    //    position.
    // Clouds: each band shares 2-3 rows with the next so the puffs
    // visibly overlap, and CLOUDS_4 reaches into the mountain region
    // (rows 33+) so the nearest cloud sweeps in front of distant peaks.
    let world = Scene::new()
        .add(winter_glow)
        .overlay(stars, scene_h, -(scene_h as i32))
        .overlay(clouds_1, 5, -22) // rows 22-26
        .overlay(clouds_2, 5, -20) // rows 24-28  (overlap rows 24-26)
        .overlay(clouds_3, 6, -18) // rows 26-31  (overlap rows 26-28)
        .overlay(clouds_4, 7, -16) // rows 28-34  (overlap rows 28-31, reaches row 34 into mountains)
        // XBin clouds — both 80×25 art layers, dropped a few rows
        // toward the horizon so the puffs sit nearer the mountain
        // line instead of the zenith.
        .overlay(xbin_cloud_2, 25, -34) // rows 10-34
        .overlay(xbin_cloud_1, 25, -29) // rows 15-39 — bottom edge tucks into mountain area
        .overlay(far_mountains, 9, -11)
        .overlay(mountains, 7, -9)
        .overlay(ground, 3, -3)
        // Tree baselines step down 2 rows per layer (37 → 39 → 41 → 43):
        // distant trees stand at the horizon, each closer layer roots
        // deeper, front trees at the very bottom edge of the scene.
        .overlay(distant_trees, 3, -9) // rows 35-37
        .overlay(far_mid_trees, 4, -8) // rows 36-39
        .overlay(mid_trees, 6, -8)     // rows 36-41
        .overlay(trees, 8, -8)         // rows 36-43
        .overlay_halo(title, 7, -13, 1, 0.75);

    // ── Outer scene: BlackBg base + a viewport_h-tall PanDown
    //    window scrolling from y=0 (top of scene = zenith stars)
    //    to y=(scene_h - viewport_h) (bottom of scene = horizon
    //    with mountains/landscape visible).
    Scene::new()
        .add(BlackBg { height: viewport_h })
        .overlay(
            PanDown::new(
                world,
                scene_h,
                hold_secs,                      // static sky hold
                2.0,                            // pan duration
                0,                              // start: top of scene
                (scene_h - viewport_h) as i32,  // end:   bottom of scene
            ),
            viewport_h,
            -(viewport_h as i32),
        )
        .quit_key('q')
        .run(30.0)
        .await;
}
