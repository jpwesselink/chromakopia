//! The scroll text runs the show. Control codes in curly braces — like
//! `{logo}` or `{pause 2}` — fire the moment their spot in the text
//! passes the middle of the screen.
//!
//! The text is parsed once into plain characters plus events, then turned
//! into a `Timeline` for one screen width: storyboard time → how far the
//! text has scrolled, and when each code fires. Everything on screen is a
//! function of time, so skipping around always lands in a consistent
//! state.

use chromakopia::animate::framebuffer::FPS;

/// Pixels per character in the scroller font.
const CHAR_PX: f64 = 8.0;
/// Scroll speed, pixels per second (one pixel per frame).
const SPEED: f64 = FPS;

/// A control code in the scroll text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Code {
    /// CHROMAKOPIA drops in at the centre; the tagline types out under it.
    Logo,
    /// The text stands still for this many seconds (the wave keeps going).
    Pause(f64),
    /// Half a screen of empty space — so what comes before has scrolled
    /// off (or what comes after is still waiting off the right edge) when
    /// a code next to it reaches the middle. Becomes spaces in `fit`.
    Gap,
}

/// The scroll text without its codes, and where each code sits.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub text: String,
    /// (character index in `text`, code) in text order.
    pub events: Vec<(usize, Code)>,
}

/// Split `src` into plain text and codes. Unknown or malformed codes are
/// an error — the text is fixed, so this fails loudly at startup.
pub fn parse(src: &str) -> Result<Script, String> {
    let mut text = String::new();
    let mut events = Vec::new();
    let mut rest = src;
    while let Some(open) = rest.find('{') {
        text.push_str(&rest[..open]);
        let close = rest[open..].find('}').ok_or_else(|| format!("unclosed code: {}", &rest[open..]))? + open;
        let body = &rest[open + 1..close];
        let mut words = body.split_whitespace();
        let code = match (words.next(), words.next(), words.next()) {
            (Some("logo"), None, _) => Code::Logo,
            (Some("gap"), None, _) => Code::Gap,
            (Some("pause"), Some(secs), None) => {
                Code::Pause(secs.parse().map_err(|_| format!("bad seconds in {{{body}}}"))?)
            }
            _ => return Err(format!("unknown code {{{body}}}")),
        };
        events.push((text.chars().count(), code));
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    Ok(Script { text, events })
}

impl Script {
    /// The script for a screen `width` columns wide: every `{gap}` turned
    /// into half a screen of spaces (whole characters, rounded up).
    pub fn fit(&self, width: usize) -> Script {
        let gap = " ".repeat((width as f64 / 2.0 / CHAR_PX).ceil() as usize);
        let chars: Vec<char> = self.text.chars().collect();
        let (mut text, mut events, mut from) = (String::new(), Vec::new(), 0);
        for &(i, code) in &self.events {
            text.extend(&chars[from..i]);
            from = i;
            match code {
                Code::Gap => text.push_str(&gap),
                _ => events.push((text.chars().count(), code)),
            }
        }
        text.extend(&chars[from..]);
        Script { text, events }
    }
}

/// When a code fires.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue {
    /// Storyboard seconds.
    pub t: f64,
    pub code: Code,
}

/// The script played on a screen `width` columns wide.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub cues: Vec<Cue>,
    /// Seconds until the text has fully scrolled off and the show loops.
    pub total: f64,
    /// Pauses as (scrolled px, start time, seconds), in order.
    pauses: Vec<(f64, f64, f64)>,
}

impl Timeline {
    pub fn new(script: &Script, width: usize) -> Self {
        let mut cues = Vec::new();
        let mut pauses = Vec::new();
        let mut paused = 0.0;
        for &(i, code) in &script.events {
            // A glyph at text pixel p is at screen x = width - scrolled + p,
            // so it reaches the middle once scrolled = p + width / 2.
            let px = i as f64 * CHAR_PX + width as f64 / 2.0;
            let t = px / SPEED + paused;
            cues.push(Cue { t, code });
            if let Code::Pause(secs) = code {
                pauses.push((px, t, secs));
                paused += secs;
            }
        }
        let run = script.text.chars().count() as f64 * CHAR_PX + width as f64;
        Self { cues, total: run / SPEED + paused, pauses }
    }

    /// Pixels the text has scrolled at time `t` (0 = just off the right edge).
    pub fn scrolled(&self, t: f64) -> f64 {
        let mut paused = 0.0;
        for &(px, start, secs) in &self.pauses {
            if t < start {
                break;
            }
            if t < start + secs {
                return px;
            }
            paused += secs;
        }
        (t - paused) * SPEED
    }

    /// When the latest `{logo}` at or before `t` fired.
    pub fn logo_since(&self, t: f64) -> Option<f64> {
        self.cues.iter().rev().find(|c| c.code == Code::Logo && c.t <= t).map(|c| c.t)
    }

    /// The next cue strictly after `t`, or the start when there is none.
    pub fn next_after(&self, t: f64) -> f64 {
        self.cues.iter().map(|c| c.t).find(|&ct| ct > t + 1e-6).unwrap_or(0.0)
    }

    /// The cue before the one we're at, or the start. A cue only just
    /// passed counts as "the one we're at", so pressing back twice walks
    /// back two cues.
    pub fn prev_before(&self, t: f64) -> f64 {
        self.cues.iter().map(|c| c.t).filter(|&ct| ct < t - 0.5).last().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_codes_and_records_their_positions() {
        let s = parse("ab{logo}cd{pause 2}e").unwrap();
        assert_eq!(s.text, "abcde");
        assert_eq!(s.events, vec![(2, Code::Logo), (4, Code::Pause(2.0))]);
    }

    #[test]
    fn parse_rejects_unknown_and_broken_codes() {
        assert!(parse("a{boom}b").unwrap_err().contains("boom"));
        assert!(parse("a{pause}b").is_err());
        assert!(parse("a{pause x}b").is_err());
        assert!(parse("a{logo").is_err());
    }

    #[test]
    fn gap_becomes_half_a_screen_of_spaces() {
        let s = parse("ab{gap}{logo}c").unwrap().fit(32);
        assert_eq!(s.text, "ab  c");
        assert_eq!(s.events, vec![(4, Code::Logo)]);
        // Rounds up to whole characters.
        assert_eq!(parse("{gap}x").unwrap().fit(20).text, "  x");
    }

    // 10 characters (80 px) on a 100-column screen: a glyph at text pixel
    // p reaches the middle when the text has scrolled p + 50.
    fn timeline() -> Timeline {
        Timeline::new(&parse("abcd{logo}efgh{pause 2}ij").unwrap(), 100)
    }

    #[test]
    fn codes_fire_when_they_reach_the_middle() {
        let tl = timeline();
        // {logo} at char 4 = 32 px → scrolled 82 → 82 / 60 s.
        assert!((tl.cues[0].t - 82.0 / 60.0).abs() < 1e-9);
        assert_eq!(tl.cues[0].code, Code::Logo);
        // {pause} at char 8 = 64 px → scrolled 114.
        assert!((tl.cues[1].t - 114.0 / 60.0).abs() < 1e-9);
    }

    #[test]
    fn pause_holds_the_text_still() {
        let tl = timeline();
        let at = 114.0 / 60.0;
        assert!((tl.scrolled(at) - 114.0).abs() < 1e-9);
        assert!((tl.scrolled(at + 1.5) - 114.0).abs() < 1e-9);
        assert!((tl.scrolled(at + 2.5) - (114.0 + 30.0)).abs() < 1e-9);
        // Whole run: 80 px of text + 100 px of screen, plus the pause.
        assert!((tl.total - (180.0 / 60.0 + 2.0)).abs() < 1e-9);
    }

    #[test]
    fn logo_shows_from_its_cue_on() {
        let tl = timeline();
        assert_eq!(tl.logo_since(1.0), None);
        assert_eq!(tl.logo_since(2.0), Some(82.0 / 60.0));
    }

    #[test]
    fn skipping_walks_the_cues_and_wraps() {
        let tl = timeline();
        let (logo, pause) = (tl.cues[0].t, tl.cues[1].t);
        assert_eq!(tl.next_after(0.0), logo);
        assert_eq!(tl.next_after(logo), pause);
        assert_eq!(tl.next_after(pause), 0.0);
        assert_eq!(tl.prev_before(pause + 0.1), logo);
        assert_eq!(tl.prev_before(logo + 0.1), 0.0);
        assert_eq!(tl.prev_before(pause + 3.0), pause);
    }
}
