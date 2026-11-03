//! Diving into the big C: after the loop the camera zooms into the liquid
//! sloshing in it until one stroke of the C fills the screen — pink air
//! over lavender liquid, the surface between them a slow, screen-wide
//! wave — and the next stretch of the scroller rides that very surface.
//! Then back out.

use chromakopia::prelude::*;

use super::glyphs::{LETTER_EXTRUDE, LETTER_HIGHLIGHT, LETTER_LOWER, LETTER_OUTLINE, LETTER_UPPER};
use super::looping;
use super::wordmark::{self, BIG_C};

/// Seconds the zoom takes, in or out.
pub const DIVE_SECS: f64 = 1.5;
/// The big C's size in pixels, before the dive.
const BIG: f64 = 8.0 * BIG_C as f64;
/// Where the camera ends up, in glyph pixels (0..8): in the middle of the
/// C's left stroke (columns 0..3), on the surface (row 4).
const LOOK: (f64, f64) = (1.5, 4.0);
/// The stroke's width, in glyph pixels — zoomed to fill the screen, with
/// some to spare.
const STROKE: f64 = 3.0;
const SPARE: f64 = 1.15;
/// The surface swells with the zoom, but no more than this many times.
const SWELL_MAX: f64 = 8.0;

/// The liquid surface's height above the C's middle, in big-C pixels, at
/// `along` big-C pixels across, `wall` seconds in: two waves running along.
pub fn wave(along: f64, wall: f64) -> f64 {
    wordmark::SLOSH * ((along * 0.35 - 3.0 * wall).sin() + 0.6 * (along * 0.6 + 2.1 * wall).sin())
}

/// The camera on the big C, `dive` (0..=1) of the way into it.
pub struct Camera {
    /// The C's size on screen, in pixels.
    size: f64,
    /// The screen point the camera looks at, and the glyph point there.
    anchor: (f64, f64),
    look: (f64, f64),
}

impl Camera {
    /// On a screen `width` × `height` pixels, `wall` seconds in.
    pub fn new(width: usize, height: usize, dive: f64, wall: f64) -> Camera {
        let e = Easing::EaseInOut.apply(dive.clamp(0.0, 1.0));
        let (cx, cy) = looping::loop_centre(width);
        let cy = cy + wordmark::big_c_bob(wall) * (1.0 - e);
        let full = 8.0 * width as f64 * SPARE / STROKE;
        let lerp = |a: f64, b: f64| a + (b - a) * e;
        Camera {
            // Zooming by the same factor each step reads as steady.
            size: BIG * (full / BIG).max(1.0).powf(e),
            anchor: (lerp(cx, width as f64 / 2.0), lerp(cy, height as f64 / 2.0)),
            look: (lerp(4.0, LOOK.0), lerp(4.0, LOOK.1)),
        }
    }

    /// The glyph point (0..8 each way) on screen at `(x, y)`.
    pub fn glyph_at(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let k = 8.0 / self.size;
        (self.look.0 + (x - self.anchor.0) * k, self.look.1 + (y - self.anchor.1) * k)
    }

    /// How much the surface's waves are scaled on screen: with the zoom,
    /// up to SWELL_MAX.
    fn swell(&self) -> f64 {
        (self.size / BIG).min(SWELL_MAX)
    }

    /// The surface's screen row under screen column `x`.
    pub fn surface_y(&self, x: f64, wall: f64) -> f64 {
        let along = (self.glyph_at((x, 0.0)).0 - 4.0) * BIG / 8.0;
        let middle = self.anchor.1 + (4.0 - self.look.1) * self.size / 8.0;
        middle - wave(along, wall) * self.swell()
    }

    /// Paint the C through the camera: pink above the surface, lavender
    /// below, a cream edge on top of each run of ink, a dark outline round it.
    pub fn paint(&self, c: &mut Canvas, glyph: &[u8; 8], wall: f64) {
        let lit = |(gx, gy): (f64, f64)| {
            (0.0..8.0).contains(&gx) && (0.0..8.0).contains(&gy) && glyph[gy as usize] & (0x80 >> gx as usize) != 0
        };
        for x in 0..c.width {
            let surface = self.surface_y(x as f64 + 0.5, wall);
            for y in 0..c.height {
                let at = |dx: f64, dy: f64| self.glyph_at((x as f64 + 0.5 + dx, y as f64 + 0.5 + dy));
                // Ink within a pixel of here, `k` pixels up-left — the
                // outline (k = 0) and extrusion shells, as `glyphs::shade`.
                let near = |k: f64| (-1..=1).any(|dy| (-1..=1).any(|dx| lit(at(dx as f64 - k, dy as f64 - k))));
                let color = if lit(at(0.0, 0.0)) {
                    if !lit(at(0.0, -1.0)) {
                        LETTER_HIGHLIGHT
                    } else if (y as f64 + 0.5) < surface {
                        LETTER_UPPER
                    } else {
                        LETTER_LOWER
                    }
                } else if near(0.0) {
                    LETTER_OUTLINE
                } else if near(1.0) || near(2.0) {
                    LETTER_EXTRUDE
                } else {
                    continue;
                };
                c.set(x as i64, y as i64, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::glyphs::mario;
    use super::super::ROWS;
    use super::*;

    const H: usize = ROWS * 2;

    #[test]
    fn starts_on_the_big_c_in_the_loop() {
        let w = 112;
        let cam = Camera::new(w, H, 0.0, 0.0);
        let (cx, cy) = looping::loop_centre(w);
        let mid = cam.glyph_at((cx, cy + wordmark::big_c_bob(0.0)));
        assert!((mid.0 - 4.0).abs() < 1e-9 && (mid.1 - 4.0).abs() < 1e-9, "{mid:?}");
        assert!((cam.size - BIG).abs() < 1e-9);
    }

    #[test]
    fn ends_with_the_stroke_filling_the_screen() {
        let c = mario('C');
        for w in [80, 112, 200, 320] {
            let cam = Camera::new(w, H, 1.0, 0.0);
            for (x, y) in [(0, 0), (w - 1, 0), (0, H - 1), (w - 1, H - 1), (w / 2, H / 2)] {
                let (gx, gy) = cam.glyph_at((x as f64 + 0.5, y as f64 + 0.5));
                assert!(c[gy as usize] & (0x80 >> gx as usize) != 0, "{w}: ({x}, {y}) is glyph ({gx}, {gy}) — not ink");
            }
        }
    }

    #[test]
    fn the_surface_is_a_wide_slow_wave_across_the_screen() {
        let w = 112;
        let cam = Camera::new(w, H, 1.0, 0.0);
        let ys: Vec<f64> = (0..w).map(|x| cam.surface_y(x as f64, 0.0)).collect();
        let (lo, hi) = ys.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &y| (lo.min(y), hi.max(y)));
        assert!(lo > 15.0 && hi < H as f64 - 15.0, "{lo}..{hi}");
        assert!(hi - lo > 8.0, "flat: {lo}..{hi}");
        // Gentle: no step of more than a couple of pixels column to column.
        assert!(ys.windows(2).all(|p| (p[1] - p[0]).abs() < 2.0));
        // And it sloshes.
        assert!((cam.surface_y(50.0, 0.0) - cam.surface_y(50.0, 0.4)).abs() > 1.0);
    }

    #[test]
    fn diving_starts_from_the_c_as_stamped() {
        // A hair into the dive the camera draws the C just as the stamp
        // did: same outline and extrusion, give or take the odd pixel.
        let w = 112;
        let render = |paint: Box<dyn Fn(&mut Canvas) + Send>| {
            let layers = Layers::new(ROWS).layer(Layer::paint(paint));
            let mut buf = FrameBuffer::new(w, ROWS);
            layers.render(&mut buf, 0);
            (0..ROWS).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| buf.get(x, y)).collect::<Vec<_>>()
        };
        let stamped = render(Box::new(|c| {
            let (cx, cy) = looping::loop_centre(c.width);
            let at = ((cx - BIG / 2.0).round() as i64, (cy + wordmark::big_c_bob(0.0) - BIG / 2.0).round() as i64);
            super::super::glyphs::stamp_tinted(c, &mario('C'), at, BIG as i64, 2, |_, _| None);
        }));
        let dived = render(Box::new(|c| Camera::new(c.width, c.height, 0.001, 0.0).paint(c, &mario('C'), 0.0)));
        let extruded = |cells: &[chromakopia::animate::Cell]| {
            cells.iter().filter(|c| c.color == LETTER_EXTRUDE || c.bg == Some(LETTER_EXTRUDE)).count() as i64
        };
        let (a, b) = (extruded(&stamped), extruded(&dived));
        assert!(b > 0 && (a - b).abs() * 5 <= a, "extrusion: stamped {a}, dived {b}");
    }

    #[test]
    fn the_water_on_screen_is_the_surface_the_letters_ride() {
        let w = 112;
        let wall = 0.7;
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| Camera::new(c.width, c.height, 1.0, wall).paint(c, &mario('C'), wall)));
        let mut buf = FrameBuffer::new(w, ROWS);
        layers.render(&mut buf, 0);
        let px = |x: usize, y: usize| {
            let cell = buf.get(x, y / 2);
            if cell.ch == ' ' { cell.bg } else if y % 2 == 0 { Some(cell.color) } else { cell.bg }
        };
        let cam = Camera::new(w, H, 1.0, wall);
        for x in (0..w).step_by(7) {
            let s = cam.surface_y(x as f64 + 0.5, wall);
            assert_eq!(px(x, (s - 1.5) as usize), Some(LETTER_UPPER), "x {x}, surface {s}");
            assert_eq!(px(x, (s + 1.5) as usize), Some(LETTER_LOWER), "x {x}, surface {s}");
        }
    }
}
