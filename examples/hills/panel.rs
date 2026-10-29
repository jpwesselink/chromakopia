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

/// Pink → lavender → sky → mint and back. Cyclic, for plasma.
fn hero() -> Vec<Color> {
    gradient(&["#ffb3c7", "#b9a2f0", "#a8d8ff", "#a6e3a6", "#ffb3c7"]).palette(256)
}

/// Violet → lavender → pink → cream: spotlights and height.
fn brand() -> Vec<Color> {
    gradient(&["#4e306e", "#b9a2f0", "#ffb3c7", "#fff8e7"]).palette(64)
}

/// Violet → lilac → blush: the softer second voice in mixes.
fn dusk() -> Vec<Color> {
    gradient(&["#4e306e", "#8c6bb8", "#c8b4ff", "#ffe0ee"]).palette(64)
}

/// Brightness ramp for effects with fixed colours: violet → lavender →
/// pink → cream. Starts above the panel fill so even dim cells stay
/// visible.
fn glow_ramp() -> Vec<Color> {
    gradient(&["#4e306e", "#8c6bb8", "#b9a2f0", "#ffb3c7", "#fff8e7"]).palette(64)
}

/// A pastel rainbow for Rainbow: pink → peach → pistachio → mint → sky →
/// lilac → back.
fn hue_loop() -> Vec<Color> {
    gradient(&["#ffb3c7", "#ffd4b8", "#d8f0a8", "#a6e3a6", "#a8d8ff", "#c8b4ff", "#ffb3c7"]).palette(360)
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
        K::Dycp => Box::new(Dycp::new(text).palette(hero())),
        K::Fld => Box::new(Fld::new(hero_plasma(text))),
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
        let text = super::super::figlet::fit("FLAP", geometry(200, 3).w - 2).join("\n");
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
    fn every_panel_has_no_warm_rainbow_hues() {
        use super::super::stage::{storyboard, StepKind};
        // Every panel — fixed-colour effects included — speaks the palette.
        for st in storyboard() {
            let StepKind::Panel { kind, name, .. } = st.kind else { continue };
            let fx = effect_for(kind, &super::super::figlet::fit(name, 100).join("\n"));
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
                        let warm = (h < 70.0 || h > 330.0) && sat > 0.45 && v > 0.35;
                        assert!(!warm, "{name} frame {frame}: warm colour {:?} at ({x},{y})", cell.color);
                    }
                }
            }
        }
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
