//! The cold open, before the scroller starts: four lines, each a step
//! louder, each on one of the pad's swells (every 4 seconds). The beat
//! comes in on the fifth swell, at `BEAT_AT`, as "Welly well" reaches the
//! middle.
//!
//! 1. "hey there" — hard on, hard off.
//! 2. "how nice of you to come along" — fading in and out
//!    (`AlphaIn` / `AlphaOut`).
//! 3. "we've been expecting you" — in the terminal's own font, easing in
//!    from the right, holding in the middle, easing out to the left.
//! 4. "let's see if we can make this more entertaining, shall we?" — in
//!    the terminal's own font too, in the letters' colours, coming in from
//!    above like the greetings' long wave: letter after letter drops from
//!    the top and bounces on the line before it settles, so the line rolls down the
//!    screen as one long wave; it lies still to be read; then the wave
//!    rolls back up and off, right to left, clearing the way for the
//!    scroller coming in from the right.
//!
//! All four are a library `Timeline` of cell effects around the
//! scroller's line.

use chromakopia::animate::framebuffer::FPS;
use chromakopia::prelude::*;

use chromakopia::animate::Cell;

use super::glyphs;

const LINE1: &str = "hey there";
const LINE2: &str = "how nice of you to come along";
const LINE3: &str = "we've been expecting you";
const LINE4: &str = "let's see if we can make this more entertaining, shall we?";

/// The pad swells every 4 seconds; a line to each.
const STEP1: (f64, f64) = (0.3, 3.0);
const STEP2: (f64, f64) = (4.2, 7.6);
const FADE_SECS: f64 = 0.8;
const STEP3_START: f64 = 8.2;
/// Step 3: seconds easing in, holding in the middle, easing out.
const IN_SECS: f64 = 1.2;
const HOLD_SECS: f64 = 1.2;
const OUT_SECS: f64 = 0.8;
/// Step 4, the whip, comes in on the fourth swell.
pub const STEP4_START: f64 = 12.0;
/// The fifth swell: the beat. The scroller sets off so that its first
/// code — "Welly well" starting the beat — reaches the middle just
/// before it, and the beat lands on this bar.
pub const BEAT_AT: f64 = 16.0;
/// Step 4, the long wave: seconds between one letter setting off from the
/// top and the next, and each letter's fall onto the line; seconds into
/// the step it lies still until; then seconds between one letter lifting
/// off (from the right) and the next, and each letter's rise.
const DROP_STAGGER: f64 = 0.008;
const DROP_FALL: f64 = 1.0;
const DROP_REST: f64 = 2.7;
const RISE_STAGGER: f64 = 0.006;
const RISE: f64 = 0.5;
/// The row the letters fall from: just under the screen's top border.
const DROP_TOP: f64 = 1.0;

/// The cold open laid out for one screen.
pub struct Intro {
    pub timeline: Timeline,
    /// Seconds before the scroller starts.
    pub secs: f64,
    /// Seconds until the last of it has gone (line 4 may still be on its
    /// way out as the scroller sets off).
    pub shown: f64,
    /// When steps 2, 3 and 4 start — skip stops.
    pub marks: [f64; 3],
}

impl Intro {
    /// The cold open for a `width` × `height` screen, on the scroller's
    /// line: `scroll_y` is the scroller's glyph top in pixel rows.
    pub fn new(width: usize, height: usize, scroll_y: i64) -> Self {
        // Lines 1 and 2 in the terminal's own font, centred across, on the
        // middle of the scroller's letters.
        let row = (scroll_y.max(0) as usize + 4) / 2;
        let centred = |line: &str| text(line, glyphs::LETTER_HIGHLIGHT).pad(row, 0, 0, width.saturating_sub(line.len()) / 2);
        let step3_end = STEP3_START + IN_SECS + HOLD_SECS + OUT_SECS;
        let step4_end = STEP4_START + Drop::secs(LINE4.chars().count());
        let timeline = Timeline::from_buffer(FrameBuffer::new(width, height), step4_end)
            .at(STEP1.0..STEP1.1, centred(LINE1))
            .at(STEP2.0..STEP2.1, centred(LINE2))
            .at(STEP2.0..STEP2.0 + FADE_SECS, AlphaIn::new(FADE_SECS))
            .at(STEP2.1 - FADE_SECS..STEP2.1, AlphaOut::new(FADE_SECS))
            .at(STEP3_START..step3_end, Slide { inner: Box::new(text(LINE3, glyphs::LETTER_HIGHLIGHT)), width, top: row as i64 })
            .at(STEP4_START..step4_end, Drop { line: LINE4, width, row });
        // The scroller's first code reaches the middle half a screen after
        // it sets off (a pixel a frame); a hair before the beat's bar.
        let secs = (BEAT_AT - 0.05 - width as f64 / 2.0 / FPS).max(step3_end + 0.2);
        let shown = step4_end.max(secs);
        Self { timeline, secs, shown, marks: [STEP2.0, STEP3_START, STEP4_START] }
    }
}

/// `line` centred on row `row` of a screen `width` wide, coming in from
/// above on a long wave — letter after letter falling from the top and
/// easing down onto the line — lying still, then rising back up and off,
/// right to left.
struct Drop {
    line: &'static str,
    width: usize,
    row: usize,
}

impl Drop {
    /// Seconds from the first letter setting off to the last one gone.
    fn secs(len: usize) -> f64 {
        DROP_REST + len.saturating_sub(1) as f64 * RISE_STAGGER + RISE
    }

    /// Row of letter `i` of `len`, `t` seconds in (None: off the top).
    fn y(&self, i: usize, len: usize, t: f64) -> Option<f64> {
        let (top, row) = (DROP_TOP, self.row as f64);
        let fall = (t - i as f64 * DROP_STAGGER) / DROP_FALL;
        if fall < 0.0 {
            return None;
        }
        let rise = (t - DROP_REST - (len - 1 - i) as f64 * RISE_STAGGER) / RISE;
        let y = if rise > 0.0 {
            // Lifting off: slow, then away, past the top.
            row - (row - top + 2.0) * rise.min(1.0).powi(3)
        } else {
            // Falling from the top and bouncing on the line before it settles.
            top + (row - top) * Easing::BounceOut.apply(fall.min(1.0))
        };
        (y >= 0.0).then_some(y)
    }

    /// Letter `i`'s colour: a calm run along the line, pink to cream to
    /// lavender.
    fn colour(i: usize, len: usize) -> Color {
        let k = 2.0 * i as f64 / (len.max(2) - 1) as f64;
        if k < 1.0 {
            Color::lerp_rgb(glyphs::LETTER_UPPER, glyphs::LETTER_HIGHLIGHT, k)
        } else {
            Color::lerp_rgb(glyphs::LETTER_HIGHLIGHT, glyphs::LETTER_LOWER, k - 1.0)
        }
    }
}

impl Effect for Drop {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let t = frame as f64 / FPS;
        let len = self.line.chars().count();
        let x0 = self.width.saturating_sub(len) / 2;
        for (i, ch) in self.line.chars().enumerate() {
            let Some(y) = self.y(i, len, t) else { continue };
            let (x, y) = (x0 + i, y.round() as usize);
            if ch != ' ' && x < buf.width && y < buf.height {
                buf.set(x, y, Cell::new(ch, Self::colour(i, len)));
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.row + 1)
    }
}

/// `inner` with its top on row `top`: easing in from just off the right
/// edge to the middle, holding there, easing out past the left edge.
struct Slide {
    inner: Box<dyn Effect>,
    width: usize,
    top: i64,
}

impl Slide {
    /// Left column of the content `t` seconds in.
    fn x(&self, t: f64) -> i64 {
        let iw = self.inner.size().0 as f64;
        let (right, mid, gone) = (self.width as f64, (self.width as f64 - iw) / 2.0, -iw);
        let x = if t < IN_SECS {
            right + (mid - right) * Easing::EaseOut.apply(t / IN_SECS)
        } else if t < IN_SECS + HOLD_SECS {
            mid
        } else {
            mid + (gone - mid) * Easing::EaseIn.apply(((t - IN_SECS - HOLD_SECS) / OUT_SECS).min(1.0))
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
        for y in 0..ih {
            for x in 0..iw {
                let cell = sub.get(x, y);
                let (tx, ty) = (x0 + x as i64, self.top + y as i64);
                if cell.ch != ' ' && tx >= 0 && ty >= 0 && (tx as usize) < buf.width && (ty as usize) < buf.height {
                    buf.set(tx as usize, ty as usize, cell);
                }
            }
        }
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.top as usize + self.inner.size().1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 112;
    const H: usize = 50;
    /// The scroller's glyph top, in pixel rows.
    const SCROLL_Y: i64 = 74;

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

    #[test]
    fn step_one_is_a_hard_cut_in_the_middle() {
        let intro = Intro::new(W, H, SCROLL_Y);
        assert_eq!(text_of(&frame_at(&intro, 0.2)), "");
        let buf = frame_at(&intro, 1.0);
        assert_eq!(text_of(&buf), LINE1);
        // On the scroller's line: the middle of its 8-pixel letters.
        assert_eq!(line_at(&buf, LINE1), Some(((W - LINE1.len()) / 2, (SCROLL_Y as usize + 4) / 2)));
        // Full colour from the first frame on: no fade.
        let (x, y) = line_at(&buf, LINE1).unwrap();
        assert_eq!(frame_at(&intro, STEP1.0).get(x, y).color, buf.get(x, y).color);
        assert_eq!(text_of(&frame_at(&intro, STEP1.1 + 0.2)), "");
    }

    #[test]
    fn step_two_fades_in_and_out() {
        let intro = Intro::new(W, H, SCROLL_Y);
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
    fn step_three_scrolls_in_holds_and_scrolls_out() {
        let intro = Intro::new(W, H, SCROLL_Y);
        assert_eq!(intro.marks, [STEP2.0, STEP3_START, STEP4_START]);
        let row = (SCROLL_Y as usize + 4) / 2;
        // Where the line's first letter is, if on screen.
        let at = |t: f64| -> Option<usize> {
            let buf = frame_at(&intro, t);
            (0..W).find(|&x| buf.get(x, row).ch != ' ')
        };
        // Starts off the right edge; scrolls in easing out — big steps
        // first, small ones as it settles.
        assert_eq!(text_of(&frame_at(&intro, STEP3_START)), "");
        let xs: Vec<usize> = (1..(IN_SECS * FPS) as usize).filter_map(|f| at(STEP3_START + f as f64 / FPS)).collect();
        assert!(xs.windows(2).all(|w| w[1] <= w[0]), "always leftward: {xs:?}");
        assert!(xs[0] - xs[3] > xs[xs.len() - 4] - xs[xs.len() - 1], "eases out: {xs:?}");
        // Holds still, whole and centred, in the terminal's own font.
        let held = STEP3_START + IN_SECS + 0.3;
        let buf = frame_at(&intro, held);
        assert_eq!(line_at(&buf, LINE3), Some(((W - LINE3.len()) / 2, row)));
        assert_eq!(at(held + 0.5), at(held));
        // Then leaves to the left, gone before line 4 comes in.
        assert_eq!(text_of(&frame_at(&intro, STEP4_START - 0.02)), "");
    }

    /// Rows the line's letters are on, `t` seconds into the cold open.
    fn rows_at(intro: &Intro, t: f64) -> std::collections::BTreeSet<usize> {
        let buf = frame_at(intro, t);
        (0..H).filter(|&y| (0..W).any(|x| buf.get(x, y).ch != ' ')).collect()
    }

    #[test]
    fn step_four_comes_down_from_above_on_a_long_wave_and_goes_back_up() {
        let intro = Intro::new(W, H, SCROLL_Y);
        let row = (SCROLL_Y as usize + 4) / 2;
        let len = LINE4.len();
        let drop = Drop { line: LINE4, width: W, row };
        // Coming in: spread down the screen from near the top, never below the line.
        let early = rows_at(&intro, STEP4_START + 0.4);
        assert!(early.len() >= 8 && *early.iter().next().unwrap() <= 6 && *early.iter().last().unwrap() <= row, "{early:?}");
        // One long wave, not scattered letters: neighbours close together.
        for f in 0..(Drop::secs(len) * FPS) as usize {
            let t = f as f64 / FPS;
            for i in 1..len {
                if let (Some(a), Some(b)) = (drop.y(i - 1, len, t), drop.y(i, len, t)) {
                    assert!((a.round() - b.round()).abs() <= 2.0, "letters {} and {i} at {t:.2}s: rows {a:.1} and {b:.1}", i - 1);
                }
            }
        }
        // Lying still, whole and centred, for over a second.
        let landed = STEP4_START + (len - 1) as f64 * DROP_STAGGER + DROP_FALL;
        assert!(STEP4_START + DROP_REST - landed >= 1.2);
        for t in [landed + 0.05, STEP4_START + DROP_REST - 0.05] {
            assert_eq!(line_at(&frame_at(&intro, t), LINE4), Some(((W - len) / 2, row)), "at {t}");
        }
        // Going: back up, the right end first.
        let going = STEP4_START + DROP_REST + 0.3;
        let buf = frame_at(&intro, going);
        let first_up = (0..len).rev().find(|&i| (0..row).any(|y| buf.get((W - len) / 2 + i, y).ch != ' ')).unwrap();
        assert!(first_up > len / 2, "lifts from the right: letter {first_up}");
        assert_eq!(text_of(&frame_at(&intro, STEP4_START + Drop::secs(len) + 0.05)), "");
        // In colour: not all one.
        let buf = frame_at(&intro, landed + 0.2);
        let colours: std::collections::BTreeSet<(u8, u8, u8)> =
            (0..W).map(|x| buf.get(x, row)).filter(|c| c.ch != ' ').map(|c| (c.color.r, c.color.g, c.color.b)).collect();
        assert!(colours.len() > 3);
    }

    #[test]
    fn every_line_comes_in_on_a_pad_swell_and_the_beat_on_the_fifth() {
        for (start, swell) in [STEP1.0, STEP2.0, STEP3_START, STEP4_START].iter().zip([0.0, 4.0, 8.0, 12.0]) {
            assert!((0.0..0.5).contains(&(start - swell)), "line at {start}, swell at {swell}");
        }
        // The scroller's first code reaches the middle just before the beat.
        for w in [80, 112, 160, 260] {
            let intro = Intro::new(w, H, SCROLL_Y);
            let first_code = intro.secs + w as f64 / 2.0 / FPS;
            assert!((BEAT_AT - 2.0..BEAT_AT).contains(&first_code), "w {w}: {first_code}");
        }
    }
}
