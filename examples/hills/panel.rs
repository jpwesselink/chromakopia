//! The styled panel each effect is shown on.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::animate::Cell;
use chromakopia::prelude::*;

use super::glyphs::{LETTER_EXTRUDE, LETTER_OUTLINE};
use super::stage::PanelKind;

/// Panel fill (the letter-outline aubergine) and its darker edge.
pub const PANEL_FILL: Color = LETTER_OUTLINE;
pub const PANEL_EDGE: Color = Color { r: 0x12, g: 0x0A, b: 0x18 };

pub const ENTER_SECS: f64 = 0.7;
pub const EXIT_SECS: f64 = 0.5;
/// The effect starts this long into the step — a beat after the panel
/// lands — and holds its first frame until then.
pub const EFFECT_DELAY: f64 = ENTER_SECS + 0.2;

const MAX_W: usize = 104;
const MIN_W: usize = 20;
const MARGIN: usize = 10;
/// Resting top row: leaves room for the heading above.
const REST_Y: i64 = 11;

/// Panel interior in cells. `y` is the resting row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub x: i64,
    pub y: i64,
    pub w: usize,
    pub h: usize,
}

pub fn geometry(screen_w: usize, block: usize) -> Geometry {
    let w = screen_w.saturating_sub(MARGIN).clamp(MIN_W, MAX_W);
    let h = if block == 4 { 15 } else { 13 };
    Geometry { x: (screen_w as i64 - w as i64) / 2, y: REST_Y, w, h }
}

/// Where a panel sits in its block's run of panels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slide {
    /// Slides in onto an empty stage (nothing to push).
    pub first: bool,
    /// Leaves by itself at the end of its step; others get pushed out.
    pub last: bool,
}

/// How far a panel travels between off-screen right, rest, and off-screen
/// left. The outgoing panel moves with the incoming one, this far behind.
pub fn travel(geo: &Geometry, screen_w: usize) -> i64 {
    screen_w as i64 - geo.x + 4
}

/// Panel left column `local` seconds into a step of `secs`: slides in
/// from the right, rests, and — if it is the last of its block — slides
/// out to the left. Always a whole cell.
pub fn slide_x(geo: &Geometry, screen_w: usize, local: f64, secs: f64, slide: Slide) -> i64 {
    let d = travel(geo, screen_w) as f64;
    let rest = geo.x as f64;
    let x = if local < ENTER_SECS {
        rest + d * (1.0 - Easing::EaseInOut.apply(local / ENTER_SECS))
    } else if slide.last && local > secs - EXIT_SECS {
        rest - d * Easing::EaseIn.apply((local - (secs - EXIT_SECS)) / EXIT_SECS)
    } else {
        rest
    };
    x.round() as i64
}

/// The effect's frame `local` seconds into a panel step.
pub fn effect_frame(local: f64) -> usize {
    ((local - EFFECT_DELAY).max(0.0) * FPS).round() as usize
}

/// Standing still: after sliding in, and before sliding out.
pub fn at_rest(local: f64, secs: f64, slide: Slide) -> bool {
    local >= ENTER_SECS && (!slide.last || local <= secs - EXIT_SECS)
}

/// Fill `[x0, x1) × [y0, y1)` in canvas pixels.
fn fill(c: &mut Canvas, (x0, y0, x1, y1): (i64, i64, i64, i64), color: Color) {
    for y in y0..y1 {
        for x in x0..x1 {
            c.set(x, y, color);
        }
    }
}

/// Two-step extrusion down-right, one-pixel edge, then the fill — the
/// letters' look, as a box, at the panel's current place.
pub fn paint_frame(c: &mut Canvas, geo: &Geometry) {
    let inner = (geo.x, geo.y * 2, geo.x + geo.w as i64, (geo.y + geo.h as i64) * 2);
    let outer = (inner.0 - 1, inner.1 - 1, inner.2 + 1, inner.3 + 1);
    for k in [2, 1] {
        fill(c, (outer.0 + k, outer.1 + k, outer.2 + k, outer.3 + k), LETTER_EXTRUDE);
    }
    fill(c, outer, PANEL_EDGE);
    fill(c, inner, PANEL_FILL);
}

/// An effect's name as panel text: the C64 font when it fits in `max_w`
/// columns, else figlet (Basic, then plain text).
pub fn name_art(name: &str, max_w: usize) -> Vec<String> {
    let rows = super::glyphs::block_text(name);
    if rows[0].chars().count() <= max_w {
        rows
    } else {
        super::figlet::fit(name, max_w)
    }
}

/// Rows of room DYCP and FLD get beyond their text: both move it up to
/// their amplitude (3) away.
const WAVE_PAD: usize = 3;

/// `inner` with `top` and `bottom` empty rows around it, so effects that
/// move rows or columns have room to do it without being cut off.
struct Pad {
    inner: Box<dyn Effect>,
    top: usize,
    bottom: usize,
}

impl Effect for Pad {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let mut sub = FrameBuffer::new(buf.width, buf.height.saturating_sub(self.top));
        self.inner.render(&mut sub, frame);
        for y in 0..sub.height {
            for x in 0..sub.width {
                let cell = sub.get(x, y);
                if cell.ch != ' ' {
                    buf.set(x, y + self.top, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        let (w, h) = self.inner.size();
        (w, h + self.top + self.bottom)
    }
}

/// DYCP over `text`: columns only dip downward, so the room goes below.
fn dycp(text: &str, pad: usize) -> Pad {
    Pad { inner: Box::new(Dycp::new(text).palette(hero())), top: 0, bottom: pad }
}

/// FLD over plasma `text`: rows bounce both ways, so room above and below.
fn fld(text: &str, pad: usize) -> Fld {
    Fld::new(Pad { inner: Box::new(hero_plasma(text)), top: pad, bottom: pad })
}

/// Plays `inner` `k` times faster.
struct Faster {
    inner: Box<dyn Effect>,
    k: usize,
}

impl Effect for Faster {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.inner.render(buf, frame * self.k);
    }

    fn size(&self) -> (usize, usize) {
        self.inner.size()
    }
}

/// Hot pink → violet → bright blue → mint and back. Cyclic, for plasma.
fn hero() -> Vec<Color> {
    gradient(&["#ff5fa2", "#9b6bff", "#3fb6ff", "#3fe0a0", "#ff5fa2"]).palette(256)
}

/// Deep violet → violet → hot pink → cream: spotlights and height.
fn brand() -> Vec<Color> {
    gradient(&["#5a3a8a", "#9b6bff", "#ff5fa2", "#fff8e7"]).palette(64)
}

/// Deep violet → purple → orchid → blush: the second voice in mixes.
fn dusk() -> Vec<Color> {
    gradient(&["#5a3a8a", "#7a4fd0", "#c58bff", "#ffc2e0"]).palette(64)
}

/// Brightness ramp for effects with fixed colours: deep violet → violet
/// → hot pink → candy pink → cream. Starts above the panel fill so even dim cells stay
/// visible.
fn glow_ramp() -> Vec<Color> {
    gradient(&["#5a3a8a", "#9b6bff", "#ff5fa2", "#ff9fc8", "#fff8e7"]).palette(64)
}

/// Rainbow in candy colours: hot pink → purple → blue → mint → back.
fn hue_loop() -> Vec<Color> {
    gradient(&["#ff5fa2", "#c45fff", "#3fb6ff", "#3fe0a0", "#ff5fa2"]).palette(360)
}

/// How `Remap` reads a cell's colour.
#[derive(Clone, Copy)]
enum By {
    /// Dark → start of the palette, bright → end.
    Luma,
    /// Hue angle → position around a cyclic palette.
    Hue,
}

/// Runs an effect that has fixed colours, then recolours its glyphs
/// through `palette` — keeping its motion and light, in the pastel palette.
struct Remap {
    inner: Box<dyn Effect>,
    palette: Vec<Color>,
    by: By,
}

impl Remap {
    fn new(inner: impl Effect, palette: Vec<Color>, by: By) -> Self {
        Self { inner: Box::new(inner), palette, by }
    }
}

impl Effect for Remap {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        self.inner.render(buf, frame);
        let last = (self.palette.len() - 1) as f64;
        for y in 0..buf.height {
            for x in 0..buf.width {
                let cell = buf.get(x, y);
                if cell.ch == ' ' {
                    continue;
                }
                let t = match self.by {
                    By::Luma => cell.color.luma(),
                    By::Hue => cell.color.to_hsv().0 / 360.0,
                };
                buf.set_color(x, y, self.palette[(t.clamp(0.0, 1.0) * last).round() as usize]);
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        self.inner.size()
    }
}

fn hero_plasma(text: &str) -> On<Plasma> {
    Plasma::new().palette(hero()).on(text)
}

/// The effect a panel shows, driven over `text` (the figlet-rendered name).
/// Everything speaks the pastel palette: effects that take one get
/// it directly, fixed-colour effects are recoloured through `Remap`.
pub fn effect_for(kind: PanelKind, text: &str) -> Box<dyn Effect> {
    use PanelKind as K;
    match kind {
        K::Plasma => Box::new(hero_plasma(text)),
        K::Rainbow => Box::new(Remap::new(Rainbow::on(text), hue_loop(), By::Hue)),
        K::Glow => Box::new(Glow::new().palette(brand()).on(text)),
        K::Pulse => Box::new(Remap::new(Pulse::on(text), glow_ramp(), By::Luma)),
        K::Neon => Box::new(Remap::new(Neon::on(text), glow_ramp(), By::Luma)),
        K::Radar => Box::new(Remap::new(Radar::on(text), glow_ramp(), By::Luma)),
        K::Karaoke => Box::new(Remap::new(Karaoke::on(text), glow_ramp(), By::Luma)),
        K::Glitch => Box::new(Remap::new(Glitch::new(text), glow_ramp(), By::Luma)),
        // Flap settles one cell every 2 frames, spaces included — far too
        // slow for a figlet block in a 5 s panel, so run it 6× faster.
        K::Flap => Box::new(Remap::new(Faster { inner: Box::new(Flap::new(text)), k: 6 }, glow_ramp(), By::Luma)),
        K::Scroll => Box::new(Scroll::new(text).palette(brand())),
        K::Spread => Box::new(Spread::new(text).palette(dusk())),
        // Whole cells only: a speed that divides the frame rate keeps every
        // step the same length (30 cells/s = one cell every 2 frames).
        K::Marquee => Box::new(Marquee::wrap(hero_plasma(text)).speed(30.0)),
        K::VSineWave => Box::new(VSineWave::wrap(hero_plasma(text))),
        K::Dycp => Box::new(dycp(text, WAVE_PAD)),
        K::Fld => Box::new(fld(text, WAVE_PAD)),
        K::Wind => Box::new(Wind::wrap(hero_plasma(text))),
        K::VectorField => Box::new(VectorField::new().palette(hero())),
        K::HeightMap => Box::new(HeightMap::new().palette(brand())),
        K::Screen => Box::new(Blend::new(hero_plasma(text), Remap::new(Radar::on(text), dusk(), By::Luma), BlendMode::Screen)),
        K::Multiply => Box::new(Blend::new(hero_plasma(text), Glow::new().palette(dusk()).on(text), BlendMode::Multiply)),
        K::Transition => Box::new(Transition::new(
            hero_plasma(text),
            Glow::new().palette(dusk()).on(text),
            2.5,
            Easing::EaseInOut,
        )),
    }
}

/// Render `effect` at `frame` and lay it over the panel interior, centred. Sized effects keep their size; unsized ones (fields) fill
/// the interior. Glyph cells get the panel fill as background; everything
/// is clipped to `buf`.
pub fn blit(buf: &mut FrameBuffer, geo: &Geometry, effect: &dyn Effect, frame: usize) {
    let (ew, eh) = effect.size();
    let (w, h) = if ew == 0 || eh == 0 { (geo.w, geo.h) } else { (ew.min(geo.w), eh.min(geo.h)) };
    let mut sub = FrameBuffer::new(w, h);
    effect.render(&mut sub, frame);
    let ox = geo.x + (geo.w - w) as i64 / 2;
    let oy = geo.y + (geo.h - h) as i64 / 2;
    for sy in 0..h {
        for sx in 0..w {
            let cell = sub.get(sx, sy);
            if cell.ch == ' ' {
                continue;
            }
            let (tx, ty) = (ox + sx as i64, oy + sy as i64);
            if tx >= 0 && ty >= 0 && (tx as usize) < buf.width && (ty as usize) < buf.height {
                buf.set(tx as usize, ty as usize, Cell { bg: Some(PANEL_FILL), ..cell });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flap_spells_its_name_before_it_falls_out() {
        use super::super::stage::PANEL_SECS;
        let text = name_art("FLAP", geometry(200, 3).w - 2).join("\n");
        let fx = effect_for(PanelKind::Flap, &text);
        let (w, h) = fx.size();
        let mut buf = FrameBuffer::new(w, h);
        // Half a second before the panel starts to fall out.
        fx.render(&mut buf, ((PANEL_SECS - EXIT_SECS - 0.5) * 60.0) as usize);
        for (y, row) in text.lines().enumerate() {
            let got: String = (0..w).map(|x| buf.get(x, y).ch).collect();
            assert_eq!(got.trim_end(), row.trim_end(), "row {y}");
        }
    }

    #[test]
    fn every_panel_skips_red_orange_and_yellow() {
        use super::super::stage::{storyboard, StepKind};
        // Every panel — fixed-colour effects included — speaks the palette.
        for st in storyboard() {
            let StepKind::Panel { kind, name, .. } = st.kind else { continue };
            let fx = effect_for(kind, &name_art(name, 100).join("\n"));
            for frame in [0, 40, 90, 150] {
                let mut buf = FrameBuffer::new(100, 13);
                fx.render(&mut buf, frame);
                for y in 0..13 {
                    for x in 0..100 {
                        let cell = buf.get(x, y);
                        if cell.ch == ' ' {
                            continue;
                        }
                        let (h, sat, v) = cell.color.to_hsv();
                        // Pinks and magentas (> 330°) are the palette; red to yellow are not.
                        let warm = (h < 60.0 || h > 345.0) && sat > 0.45 && v > 0.35;
                        assert!(!warm, "{name} frame {frame}: warm colour {:?} at ({x},{y})", cell.color);
                    }
                }
            }
        }
    }

    #[test]
    fn names_use_the_c64_font_and_fall_back_when_too_wide() {
        assert_eq!(name_art("VECTORFIELD", 102), super::super::glyphs::block_text("VECTORFIELD"));
        let narrow = name_art("VECTORFIELD", 60);
        assert!(narrow.iter().all(|r| r.chars().count() <= 60), "{narrow:?}");
    }

    #[test]
    fn dycp_and_fld_keep_all_their_ink_inside_their_size() {
        // The waves move rows and columns beyond the text. Rendered into a
        // far taller buffer, nothing more may show up than within their
        // own size — i.e. nothing hangs off the bottom. (Off the top can't
        // happen: FLD's text starts WAVE_PAD rows down, its amplitude is 3,
        // and DYCP only dips downward.)
        let text = name_art("DYCP", 100).join("\n");
        let ink = |fx: &dyn Effect, extra: usize, frame| {
            let (w, h) = fx.size();
            let mut buf = FrameBuffer::new(w, h + extra);
            fx.render(&mut buf, frame);
            (0..h + extra).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| buf.get(x, y).ch != ' ').count()
        };
        let (dycp, fld) = (dycp(&text, WAVE_PAD), fld(&text, WAVE_PAD));
        for frame in (0..600).step_by(7) {
            assert_eq!(ink(&dycp, 0, frame), ink(&dycp, 20, frame), "dycp frame {frame}");
            assert_eq!(ink(&fld, 0, frame), ink(&fld, 20, frame), "fld frame {frame}");
        }
        // Unpadded, they do lose ink — the bug this guards.
        let bare = Dycp::new(&text).palette(hero());
        assert!((0..600).step_by(7).any(|f| ink(&bare, 0, f) < ink(&bare, 20, f)));
        assert!(dycp.size().1 <= 13 && fld.size().1 <= 13, "fits the panel");
    }

    #[test]
    fn effect_holds_its_first_frame_until_after_landing() {
        assert_eq!(effect_frame(0.0), 0);
        assert_eq!(effect_frame(ENTER_SECS), 0);
        assert_eq!(effect_frame(EFFECT_DELAY), 0);
        assert_eq!(effect_frame(EFFECT_DELAY + 1.0), 60);
    }

    #[test]
    fn geometry_is_centred() {
        let g = geometry(200, 2);
        assert_eq!((g.w, g.h, g.y), (104, 13, 11));
        assert_eq!(g.x, (200 - 104) / 2);
        assert_eq!(geometry(200, 4).h, 15);
    }

    #[test]
    fn geometry_clamps_to_narrow_screen() {
        let g = geometry(60, 2);
        assert_eq!(g.w, 50);
        assert_eq!(g.x, 5);
        assert_eq!(geometry(10, 2).w, 20); // never below the minimum
    }

    #[test]
    fn panel_slides_in_from_the_right_and_out_to_the_left() {
        let g = geometry(200, 2);
        let mid = Slide { first: true, last: false };
        let last = Slide { first: false, last: true };
        // Starts fully off the right edge (outline at x - 1).
        assert!(slide_x(&g, 200, 0.0, 5.0, mid) - 1 >= 200);
        assert_eq!(slide_x(&g, 200, ENTER_SECS, 5.0, mid), g.x);
        // A panel that gets pushed doesn't leave by itself.
        assert_eq!(slide_x(&g, 200, 4.9, 5.0, mid), g.x);
        // The last of a block leaves fully off the left edge (extrusion to x + w + 3).
        assert!(slide_x(&g, 200, 5.0, 5.0, last) + g.w as i64 + 3 <= 0);
        assert!(at_rest(2.5, 5.0, mid) && !at_rest(0.1, 5.0, mid));
        assert!(at_rest(4.9, 5.0, mid) && !at_rest(4.9, 5.0, last));
    }

    #[test]
    fn pushed_panel_leaves_by_the_same_distance() {
        let g = geometry(200, 2);
        let s = Slide { first: false, last: false };
        // At the start of the push the outgoing panel is where it rested,
        // at the end it is fully off the left edge.
        assert_eq!(slide_x(&g, 200, 0.0, 5.0, s) - travel(&g, 200), g.x);
        assert!(slide_x(&g, 200, ENTER_SECS, 5.0, s) - travel(&g, 200) + g.w as i64 + 3 <= 0);
    }

    #[test]
    fn blit_paints_glyphs_on_panel_fill() {
        let g = Geometry { x: 2, y: 1, w: 6, h: 3 };
        let mut buf = FrameBuffer::new(10, 5);
        blit(&mut buf, &g, &text("X", Color::new(255, 0, 0)), 0);
        // 1×1 content centred in a 6×3 interior: column 2 + 5/2 = 4, row 1 + 2/2 = 2.
        let cell = buf.get(4, 2);
        assert_eq!((cell.ch, cell.bg), ('X', Some(PANEL_FILL)));
    }

    #[test]
    fn blit_clips_to_small_buffer() {
        let g = geometry(200, 2);
        let mut buf = FrameBuffer::new(30, 4); // far smaller than the panel
        blit(&mut buf, &g, &text("WIDE TEXT", Color::new(255, 255, 255)), 0);
    }

    #[test]
    fn every_kind_builds_and_renders() {
        use super::super::stage::{storyboard, StepKind};
        for st in storyboard() {
            if let StepKind::Panel { kind, name, .. } = st.kind {
                let fx = effect_for(kind, name);
                let mut buf = FrameBuffer::new(60, 12);
                fx.render(&mut buf, 30);
            }
        }
    }
}
