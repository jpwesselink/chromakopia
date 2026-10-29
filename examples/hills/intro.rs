//! The cold open, before the scroller starts: three "welly"s, each a step
//! louder, on an empty screen — built as a library `Timeline` of cell
//! effects.
//!
//! 1. "welly", hard on, hard off.
//! 2. "welly", fading in and out (`AlphaIn` / `AlphaOut`).
//! 3. "welly", big block letters in a wild plasma, sliding in from the
//!    right, holding in the middle, sliding out to the left.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;

use super::glyphs;

/// Seconds the cold open lasts; the scroller starts after it.
pub const SECS: f64 = 12.0;
/// When steps 2 and 3 start — skip stops.
pub const MARKS: [f64; 2] = [STEP2.0, STEP3.0];

const STEP1: (f64, f64) = (0.5, 2.0);
const STEP2: (f64, f64) = (2.8, 6.3);
const FADE_SECS: f64 = 0.8;
const STEP3: (f64, f64) = (7.0, 11.4);
const SLIDE_SECS: f64 = 1.2;

/// The wild plasma for step 3: hot pink, sun yellow, ice cyan, violet.
fn wild() -> Vec<Color> {
    gradient(&["#ff2bd6", "#ffd23f", "#3fe0ff", "#9b5cff", "#ff2bd6"]).palette(256)
}

/// The cold open for a screen `width` × `height` cells.
pub fn cold_open(width: usize, height: usize) -> Timeline {
    // "welly" in the terminal's own font, dead centre.
    let welly = || text("welly", glyphs::LETTER_HIGHLIGHT).pad(height / 2, 0, 0, width.saturating_sub(5) / 2);
    // Each ROM pixel 2×2 cells: big, and square-ish on tall terminal cells.
    let rows: Vec<String> = glyphs::block_text("welly")
        .iter()
        .map(|r| r.chars().flat_map(|c| [c, c]).collect::<String>())
        .flat_map(|r| [r.clone(), r])
        .collect();
    let big = Plasma::new().palette(wild()).on(&rows.join("\n"));
    Timeline::from_buffer(FrameBuffer::new(width, height), SECS)
        .at(STEP1.0..STEP1.1, welly())
        .at(STEP2.0..STEP2.1, welly())
        .at(STEP2.0..STEP2.0 + FADE_SECS, AlphaIn::new(FADE_SECS))
        .at(STEP2.1 - FADE_SECS..STEP2.1, AlphaOut::new(FADE_SECS))
        .at(STEP3.0..STEP3.1, Slide { inner: Box::new(big), width, height, secs: STEP3.1 - STEP3.0 })
}

/// `inner`, vertically centred, sliding in from the right edge to the
/// middle (ease-out), holding there, and sliding out past the left edge
/// (ease-in) — over `secs` in all.
struct Slide {
    inner: Box<dyn Effect>,
    width: usize,
    height: usize,
    secs: f64,
}

impl Slide {
    /// Left column of the content `t` seconds in.
    fn x(&self, t: f64) -> i64 {
        let iw = self.inner.size().0 as f64;
        let (right, mid, gone) = (self.width as f64, (self.width as f64 - iw) / 2.0, -iw);
        let x = if t < SLIDE_SECS {
            right + (mid - right) * Easing::EaseOut.apply(t / SLIDE_SECS)
        } else if t > self.secs - SLIDE_SECS {
            mid + (gone - mid) * Easing::EaseIn.apply((t - (self.secs - SLIDE_SECS)) / SLIDE_SECS)
        } else {
            mid
        };
        x.round() as i64
    }
}

impl Effect for Slide {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let (iw, ih) = self.inner.size();
        let mut sub = FrameBuffer::new(iw, ih);
        self.inner.render(&mut sub, frame);
        let x0 = self.x(frame as f64 / FPS);
        let y0 = self.height.saturating_sub(ih) as i64 / 2;
        for y in 0..ih {
            for x in 0..iw {
                let cell = sub.get(x, y);
                let (tx, ty) = (x0 + x as i64, y0 + y as i64);
                if cell.ch != ' ' && tx >= 0 && ty >= 0 && (tx as usize) < buf.width && (ty as usize) < buf.height {
                    buf.set(tx as usize, ty as usize, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_at(t: f64) -> FrameBuffer {
        let mut buf = FrameBuffer::new(112, 50);
        cold_open(112, 50).render(&mut buf, (t * FPS).round() as usize);
        buf
    }

    fn text_of(buf: &FrameBuffer) -> String {
        (0..buf.height)
            .map(|y| (0..buf.width).map(|x| buf.get(x, y).ch).collect::<String>().trim().to_string())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Where "welly" sits on screen: (column, row) of its 'w'.
    fn welly_at(buf: &FrameBuffer) -> Option<(usize, usize)> {
        (0..buf.height).find_map(|y| {
            let line: String = (0..buf.width).map(|x| buf.get(x, y).ch).collect();
            line.find("welly").map(|x| (x, y))
        })
    }

    #[test]
    fn step_one_is_a_hard_cut_in_the_middle() {
        assert_eq!(text_of(&frame_at(0.2)), "");
        let buf = frame_at(1.0);
        assert_eq!(text_of(&buf), "welly");
        assert_eq!(welly_at(&buf), Some(((112 - 5) / 2, 50 / 2)));
        // Full colour from the first frame on: no fade.
        let (x, y) = welly_at(&frame_at(STEP1.0)).unwrap();
        assert_eq!(frame_at(STEP1.0).get(x, y).color, frame_at(1.5).get(x, y).color);
        assert_eq!(text_of(&frame_at(2.3)), "");
    }

    #[test]
    fn step_two_fades_in_and_out() {
        let luma = |t: f64| {
            let buf = frame_at(t);
            let (x, y) = welly_at(&buf).expect("welly on screen");
            buf.get(x, y).color.luma()
        };
        assert!(luma(STEP2.0 + 0.2) < luma(STEP2.0 + 1.5));
        assert!(luma(STEP2.1 - 0.2) < luma(STEP2.0 + 1.5));
        assert_eq!(text_of(&frame_at(6.6)), "");
    }

    #[test]
    fn step_three_slides_a_big_plasma_welly_through() {
        let blocks = |t: f64| -> Vec<usize> {
            let buf = frame_at(t);
            (0..buf.width).filter(|&x| (0..buf.height).any(|y| buf.get(x, y).ch == '█')).collect()
        };
        // Coming in from the right, then centred, then leaving on the left.
        let entering = blocks(STEP3.0 + 0.3);
        assert!(!entering.is_empty() && *entering.iter().min().unwrap() > 112 / 2);
        let held = blocks(STEP3.0 + SLIDE_SECS + 0.5);
        let (lo, hi) = (*held.iter().min().unwrap(), *held.iter().max().unwrap());
        assert!(((lo + hi) as i64 - 112).abs() <= 2, "held at {lo}..{hi}");
        let leaving = blocks(STEP3.1 - 0.3);
        assert!(!leaving.is_empty() && *leaving.iter().max().unwrap() < 112 / 2);
        // Plasma: more than one colour across the letters.
        let buf = frame_at(STEP3.0 + SLIDE_SECS + 0.5);
        let colours: std::collections::HashSet<(u8, u8, u8)> = (0..112)
            .flat_map(|x| (0..50).map(move |y| (x, y)))
            .filter(|&(x, y)| buf.get(x, y).ch == '█')
            .map(|(x, y)| buf.get(x, y).color)
            .map(|c| (c.r, c.g, c.b))
            .collect();
        assert!(colours.len() > 5);
        // Big: each ROM pixel is 2×2 cells.
        assert!(hi - lo + 1 >= 70, "only {} columns wide", hi - lo + 1);
        assert_eq!(text_of(&frame_at(SECS - 0.1)), "");
    }
}
