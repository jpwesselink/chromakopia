//! The cold open, before the scroller starts: three lines on an empty
//! screen, each a step louder — built as a library `Timeline` of cell
//! effects.
//!
//! 1. "hey there" — hard on, hard off.
//! 2. "how nice of you to come along" — fading in and out
//!    (`AlphaIn` / `AlphaOut`).
//! 3. "are you entertained? or would you like some more?" — cream C64
//!    letters at the scroller's size, sliding through from right to left.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;

use super::glyphs;

const LINE1: &str = "hey there";
const LINE2: &str = "how nice of you to come along";
const LINE3: &str = "are you entertained? or would you like some more?";

const STEP1: (f64, f64) = (0.5, 2.5);
const STEP2: (f64, f64) = (3.2, 7.2);
const FADE_SECS: f64 = 0.8;
const STEP3_START: f64 = 8.0;
/// Step 3's columns per frame: a whole number, so every step is the same.
const SLIDE_COLS: usize = 2;
/// Quiet after step 3 before the scroller starts.
const TAIL_SECS: f64 = 0.5;

/// `line` in the C64 ROM font at the scroller's size: one column per
/// pixel, two pixel rows per cell (`▀` `▄` `█`).
fn half_blocks(line: &str) -> String {
    let rows = glyphs::block_text(line);
    rows.chunks(2)
        .map(|pair| {
            let bottom: Vec<char> = pair.get(1).map_or(Vec::new(), |r| r.chars().collect());
            pair[0]
                .chars()
                .enumerate()
                .map(|(i, top)| match (top != ' ', bottom.get(i).is_some_and(|&b| b != ' ')) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The cold open laid out for one screen.
pub struct Intro {
    pub timeline: Timeline,
    /// Seconds it lasts; the scroller starts after it.
    pub secs: f64,
    /// When steps 2 and 3 start — skip stops.
    pub marks: [f64; 2],
}

impl Intro {
    pub fn new(width: usize, height: usize) -> Self {
        // Lines 1 and 2 in the terminal's own font, dead centre.
        let centred = |line: &str| {
            text(line, glyphs::LETTER_HIGHLIGHT).pad(height / 2, 0, 0, width.saturating_sub(line.len()) / 2)
        };
        let big = text(&half_blocks(LINE3), glyphs::LETTER_HIGHLIGHT);
        // Step 3 lasts as long as the letters take to cross the screen.
        let travel = width + big.size().0;
        let slide_secs = (travel / SLIDE_COLS + 1) as f64 / FPS;
        let step3_end = STEP3_START + slide_secs;
        let timeline = Timeline::from_buffer(FrameBuffer::new(width, height), step3_end)
            .at(STEP1.0..STEP1.1, centred(LINE1))
            .at(STEP2.0..STEP2.1, centred(LINE2))
            .at(STEP2.0..STEP2.0 + FADE_SECS, AlphaIn::new(FADE_SECS))
            .at(STEP2.1 - FADE_SECS..STEP2.1, AlphaOut::new(FADE_SECS))
            .at(STEP3_START..step3_end, Slide { inner: Box::new(big), width, height });
        Self { timeline, secs: step3_end + TAIL_SECS, marks: [STEP2.0, STEP3_START] }
    }
}

/// `inner`, vertically centred, sliding from just off the right edge
/// leftward `SLIDE_COLS` columns a frame, until it's gone off the left.
struct Slide {
    inner: Box<dyn Effect>,
    width: usize,
    height: usize,
}

impl Effect for Slide {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let (iw, ih) = self.inner.size();
        let mut sub = FrameBuffer::new(iw, ih);
        self.inner.render(&mut sub, frame);
        let x0 = self.width as i64 - (frame * SLIDE_COLS) as i64;
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

    const W: usize = 112;
    const H: usize = 50;

    fn frame_at(intro: &Intro, t: f64) -> FrameBuffer {
        let mut buf = FrameBuffer::new(W, H);
        intro.timeline.render(&mut buf, (t * FPS).round() as usize);
        buf
    }

    fn text_of(buf: &FrameBuffer) -> String {
        (0..buf.height)
            .map(|y| (0..buf.width).map(|x| buf.get(x, y).ch).collect::<String>().trim().to_string())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// (column, row) where `line` starts on screen.
    fn line_at(buf: &FrameBuffer, line: &str) -> Option<(usize, usize)> {
        (0..buf.height).find_map(|y| {
            let row: String = (0..buf.width).map(|x| buf.get(x, y).ch).collect();
            row.find(line).map(|x| (x, y))
        })
    }

    fn is_block(ch: char) -> bool {
        matches!(ch, '█' | '▀' | '▄')
    }

    /// Columns holding step 3's block letters.
    fn blocks(buf: &FrameBuffer) -> Vec<usize> {
        (0..buf.width).filter(|&x| (0..buf.height).any(|y| is_block(buf.get(x, y).ch))).collect()
    }

    #[test]
    fn step_one_is_a_hard_cut_in_the_middle() {
        let intro = Intro::new(W, H);
        assert_eq!(text_of(&frame_at(&intro, 0.2)), "");
        let buf = frame_at(&intro, 1.0);
        assert_eq!(text_of(&buf), LINE1);
        assert_eq!(line_at(&buf, LINE1), Some(((W - LINE1.len()) / 2, H / 2)));
        // Full colour from the first frame on: no fade.
        let (x, y) = line_at(&buf, LINE1).unwrap();
        assert_eq!(frame_at(&intro, STEP1.0).get(x, y).color, buf.get(x, y).color);
        assert_eq!(text_of(&frame_at(&intro, STEP1.1 + 0.2)), "");
    }

    #[test]
    fn step_two_fades_in_and_out() {
        let intro = Intro::new(W, H);
        let luma = |t: f64| {
            let buf = frame_at(&intro, t);
            let (x, y) = line_at(&buf, LINE2).expect("line 2 on screen");
            buf.get(x, y).color.luma()
        };
        let full = luma(STEP2.0 + 2.0);
        assert!(luma(STEP2.0 + 0.2) < full);
        assert!(luma(STEP2.1 - 0.2) < full);
        assert_eq!(text_of(&frame_at(&intro, STEP2.1 + 0.2)), "");
    }

    #[test]
    fn step_three_slides_big_letters_straight_through() {
        let intro = Intro::new(W, H);
        assert_eq!(intro.marks, [STEP2.0, STEP3_START]);
        // Starts off the right edge, ends off the left edge.
        assert!(blocks(&frame_at(&intro, STEP3_START)).is_empty());
        assert!(blocks(&frame_at(&intro, intro.secs - 0.05)).is_empty());
        // Coming in, its left edge moves exactly SLIDE_COLS columns a frame.
        let early = STEP3_START + 0.3;
        let (ra, rb) = (blocks(&frame_at(&intro, early)), blocks(&frame_at(&intro, early + 1.0 / FPS)));
        assert!(!ra.is_empty());
        assert_eq!(ra.iter().min().unwrap() - rb.iter().min().unwrap(), SLIDE_COLS);
        let a = frame_at(&intro, STEP3_START + (intro.secs - STEP3_START) / 2.0);
        // The scroller's size: 8 columns a letter, two pixel rows a cell —
        // 8 pixel rows (with descenders) in 4 terminal rows.
        let rows = (0..H).filter(|&y| (0..W).any(|x| is_block(a.get(x, y).ch))).count();
        assert_eq!(rows, 4);
        let colours: std::collections::HashSet<(u8, u8, u8)> = (0..W)
            .flat_map(|x| (0..H).map(move |y| (x, y)))
            .filter(|&(x, y)| is_block(a.get(x, y).ch))
            .map(|(x, y)| a.get(x, y).color)
            .map(|c| (c.r, c.g, c.b))
            .collect();
        assert_eq!(colours.len(), 1);
        assert!(colours.contains(&(glyphs::LETTER_HIGHLIGHT.r, glyphs::LETTER_HIGHLIGHT.g, glyphs::LETTER_HIGHLIGHT.b)));
    }

    #[test]
    fn longer_on_wider_screens() {
        assert!(Intro::new(200, H).secs > Intro::new(112, H).secs);
    }
}
