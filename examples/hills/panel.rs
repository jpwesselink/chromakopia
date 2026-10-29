//! The styled panel each effect is shown on.

use chromakopia::animate::Cell;
use chromakopia::prelude::*;

use super::glyphs::{LETTER_EXTRUDE, LETTER_OUTLINE};
use super::stage::PanelKind;
use super::ROWS;

/// Panel fill — the letter-outline aubergine.
pub const PANEL_FILL: Color = LETTER_OUTLINE;
/// One-pixel edge around the fill, a shade darker.
pub const PANEL_EDGE: Color = Color { r: 0x12, g: 0x0A, b: 0x18 };

pub const ENTER_SECS: f64 = 0.7;
pub const EXIT_SECS: f64 = 0.5;

const MAX_W: usize = 104;
const MIN_W: usize = 20;
const MARGIN: usize = 10;
/// Resting top row: leaves room for the heading above.
const REST_Y: i64 = 9;

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

/// The effect a panel shows, driven over `text` (the figlet-rendered name).
pub fn effect_for(kind: PanelKind, text: &str) -> Box<dyn Effect> {
    use PanelKind as K;
    match kind {
        K::Plasma => Box::new(Plasma::on(text)),
        K::Rainbow => Box::new(Rainbow::on(text)),
        K::Glow => Box::new(Glow::on(text)),
        K::Pulse => Box::new(Pulse::on(text)),
        K::Neon => Box::new(Neon::on(text)),
        K::Radar => Box::new(Radar::on(text)),
        K::Karaoke => Box::new(Karaoke::on(text)),
        K::Glitch => Box::new(Glitch::new(text)),
        K::Flap => Box::new(Flap::new(text)),
        K::Scroll => Box::new(Scroll::new(text)),
        K::Spread => Box::new(Spread::new(text)),
        K::Marquee => Box::new(Marquee::wrap(Rainbow::on(text)).speed(20.0)),
        K::VSineWave => Box::new(VSineWave::wrap(Plasma::on(text))),
        K::Dycp => Box::new(Dycp::new(text)),
        K::Fld => Box::new(Fld::new(Plasma::on(text))),
        K::Wind => Box::new(Wind::wrap(Plasma::on(text))),
        K::VectorField => Box::new(VectorField::new()),
        K::HeightMap => Box::new(HeightMap::new()),
        K::Screen => Box::new(Blend::new(Plasma::on(text), Radar::on(text), BlendMode::Screen)),
        K::Multiply => Box::new(Blend::new(Rainbow::on(text), Glow::on(text), BlendMode::Multiply)),
        K::Transition => Box::new(Transition::new(Plasma::on(text), Rainbow::on(text), 2.5, Easing::EaseInOut)),
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
    fn geometry_is_centred() {
        let g = geometry(200, 2);
        assert_eq!((g.w, g.h, g.y), (104, 13, 9));
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
