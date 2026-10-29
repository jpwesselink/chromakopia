//! The styled panel each effect is shown on.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::animate::Cell;
use chromakopia::prelude::*;

use super::glyphs::LETTER_EXTRUDE;
use super::stage::PanelKind;
use super::ROWS;

/// Panel fill and edge — the docs site's feature cards.
pub const PANEL_FILL: Color = Color { r: 0x0F, g: 0x1F, b: 0x0F };
pub const PANEL_EDGE: Color = Color { r: 0x2A, g: 0x4A, b: 0x2A };

pub const ENTER_SECS: f64 = 0.7;
pub const EXIT_SECS: f64 = 0.5;
/// The effect starts this long into the step — a beat after the panel
/// lands — and holds its first frame until then.
pub const EFFECT_DELAY: f64 = ENTER_SECS + 0.2;

const MAX_W: usize = 104;
const MIN_W: usize = 20;
const MARGIN: usize = 10;
/// Resting top row: leaves room for the heading above.
const REST_Y: i64 = 8;

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

/// Panel top row `local` seconds into a step of `secs`: bounces in from
/// above, rests, falls out below. Always a whole cell.
pub fn panel_y(geo: &Geometry, local: f64, secs: f64) -> i64 {
    let above = -(geo.h as f64) - 3.0;
    let below = ROWS as f64 + 2.0;
    let rest = geo.y as f64;
    let y = if local < ENTER_SECS {
        above + (rest - above) * Easing::BounceOut.apply(local / ENTER_SECS)
    } else if local > secs - EXIT_SECS {
        rest + (below - rest) * Easing::EaseIn.apply((local - (secs - EXIT_SECS)) / EXIT_SECS)
    } else {
        rest
    };
    y.round() as i64
}

/// The effect's frame `local` seconds into a panel step.
pub fn effect_frame(local: f64) -> usize {
    ((local - EFFECT_DELAY).max(0.0) * FPS).round() as usize
}

/// Between the bounce-in and the fall-out.
pub fn at_rest(local: f64, secs: f64) -> bool {
    local >= ENTER_SECS && local <= secs - EXIT_SECS
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
/// letters' look, as a box. `y` is the panel's current top row (cells).
pub fn paint_frame(c: &mut Canvas, geo: &Geometry, y: i64) {
    let inner = (geo.x, y * 2, geo.x + geo.w as i64, (y + geo.h as i64) * 2);
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

/// The docs site hero's banner palette (chromakopia-wasm letters_drop):
/// deep green → brand green → cyan → blue and back. Cyclic, for plasma.
fn hero() -> Vec<Color> {
    gradient(&["#003322", "#00ff88", "#00ffcc", "#00bbff", "#003322"]).palette(256)
}

/// Card green → brand green → brand-lighter: spotlights and height.
fn brand() -> Vec<Color> {
    gradient(&["#0f1f0f", "#009944", "#00cc66", "#99ffdd"]).palette(64)
}

/// The hero's muted ASCII: plum → dusty violet → pale lilac.
fn dusk() -> Vec<Color> {
    gradient(&["#2a1428", "#4e2a4e", "#5a5472", "#c8c0d8"]).palette(64)
}

fn hero_plasma(text: &str) -> On<Plasma> {
    Plasma::new().palette(hero()).on(text)
}

/// The effect a panel shows, driven over `text` (the figlet-rendered name).
/// Everything that takes a palette gets the docs site's; only the
/// RAINBOW panel shows a rainbow.
pub fn effect_for(kind: PanelKind, text: &str) -> Box<dyn Effect> {
    use PanelKind as K;
    match kind {
        K::Plasma => Box::new(hero_plasma(text)),
        K::Rainbow => Box::new(Rainbow::on(text)),
        K::Glow => Box::new(Glow::new().palette(brand()).on(text)),
        K::Pulse => Box::new(Pulse::on(text)),
        K::Neon => Box::new(Neon::on(text)),
        K::Radar => Box::new(Radar::on(text)),
        K::Karaoke => Box::new(Karaoke::on(text)),
        K::Glitch => Box::new(Glitch::new(text)),
        // Flap settles one cell every 2 frames, spaces included — far too
        // slow for a figlet block in a 5 s panel, so run it 6× faster.
        K::Flap => Box::new(Faster { inner: Box::new(Flap::new(text)), k: 6 }),
        K::Scroll => Box::new(Scroll::new(text).palette(brand())),
        K::Spread => Box::new(Spread::new(text).palette(dusk())),
        K::Marquee => Box::new(Marquee::wrap(hero_plasma(text)).speed(20.0)),
        K::VSineWave => Box::new(VSineWave::wrap(hero_plasma(text))),
        K::Dycp => Box::new(Dycp::new(text).palette(hero())),
        K::Fld => Box::new(Fld::new(hero_plasma(text))),
        K::Wind => Box::new(Wind::wrap(hero_plasma(text))),
        K::VectorField => Box::new(VectorField::new().palette(hero())),
        K::HeightMap => Box::new(HeightMap::new().palette(brand())),
        K::Screen => Box::new(Blend::new(hero_plasma(text), Radar::on(text), BlendMode::Screen)),
        K::Multiply => Box::new(Blend::new(hero_plasma(text), Glow::new().palette(dusk()).on(text), BlendMode::Multiply)),
        K::Transition => Box::new(Transition::new(
            hero_plasma(text),
            Glow::new().palette(dusk()).on(text),
            2.5,
            Easing::EaseInOut,
        )),
    }
}

/// Render `effect` at `frame` and lay it over the panel interior at row
/// `y`, centred. Sized effects keep their size; unsized ones (fields) fill
/// the interior. Glyph cells get the panel fill as background; everything
/// is clipped to `buf`.
pub fn blit(buf: &mut FrameBuffer, geo: &Geometry, y: i64, effect: &dyn Effect, frame: usize) {
    let (ew, eh) = effect.size();
    let (w, h) = if ew == 0 || eh == 0 { (geo.w, geo.h) } else { (ew.min(geo.w), eh.min(geo.h)) };
    let mut sub = FrameBuffer::new(w, h);
    effect.render(&mut sub, frame);
    let ox = geo.x + (geo.w - w) as i64 / 2;
    let oy = y + (geo.h - h) as i64 / 2;
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
    fn palette_panels_have_no_warm_rainbow_hues() {
        use super::super::stage::{storyboard, StepKind};
        use PanelKind as K;
        // Effects with their own fixed library colours keep them.
        let fixed = [K::Rainbow, K::Pulse, K::Neon, K::Radar, K::Karaoke, K::Glitch, K::Flap, K::Screen];
        for st in storyboard() {
            let StepKind::Panel { kind, name, .. } = st.kind else { continue };
            if fixed.contains(&kind) {
                continue;
            }
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
        assert_eq!((g.w, g.h, g.y), (104, 13, 8));
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
    fn panel_bounces_in_rests_and_falls_out() {
        let g = geometry(200, 2);
        assert!(panel_y(&g, 0.0, 5.0) < 0);
        assert_eq!(panel_y(&g, ENTER_SECS, 5.0), g.y);
        assert_eq!(panel_y(&g, 2.5, 5.0), g.y);
        assert!(panel_y(&g, 5.0, 5.0) >= super::super::ROWS as i64);
        assert!(at_rest(2.5, 5.0) && !at_rest(0.1, 5.0) && !at_rest(4.9, 5.0));
    }

    #[test]
    fn blit_paints_glyphs_on_panel_fill() {
        let g = Geometry { x: 2, y: 1, w: 6, h: 3 };
        let mut buf = FrameBuffer::new(10, 5);
        blit(&mut buf, &g, 1, &text("X", Color::new(255, 0, 0)), 0);
        // 1×1 content centred in a 6×3 interior: column 2 + 5/2 = 4, row 1 + 2/2 = 2.
        let cell = buf.get(4, 2);
        assert_eq!((cell.ch, cell.bg), ('X', Some(PANEL_FILL)));
    }

    #[test]
    fn blit_clips_to_small_buffer() {
        let g = geometry(200, 2);
        let mut buf = FrameBuffer::new(30, 4); // far smaller than the panel
        blit(&mut buf, &g, g.y, &text("WIDE TEXT", Color::new(255, 255, 255)), 0);
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
