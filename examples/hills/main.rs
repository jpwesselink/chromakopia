//! Hills showcase — chromakopia, cracktro style, over a pastel landscape
//! of swaying dithered hills (half-block pixel `Layers`).
//!
//! The scroll text along the bottom runs the show: control codes in it
//! (see `script.rs`) fire as they pass the middle of the screen. It opens
//! on the landscape and the scroller alone; `{logo}` drops CHROMAKOPIA in
//! and types the tagline under it. The effect panels (`stage.rs`,
//! `panel.rs`) wait for codes of their own.
//!
//! Run: `cargo run --release --example hills`
//! Keys: → n space next code · ← p previous code · q quit

mod figlet;
mod glyphs;
mod landscape;
mod layout;
mod panel;
mod script;
mod scroller;
// The storyboard's clock and seeking are idle until script codes drive
// the panels again.
#[allow(dead_code)]
mod stage;
mod tiny;
mod wordmark;

use chromakopia::prelude::*;
use std::sync::{Arc, Mutex};

use chromakopia::animate::framebuffer::FPS;
use chromakopia::animate::Cell;
#[cfg(test)]
use stage::locate;
use stage::{storyboard, Seek, Step, StepKind};
use wordmark::LogoPhase;

/// Scene height in terminal rows (two pixels each).
pub const ROWS: usize = 50;
/// Rows of open sky above the landscape, so the wordmark has headroom.
pub const TOP_PAD: usize = 6;
/// Top pixel row of the pitch scroller at rest (it waves around this).
pub const SCROLLER_Y: i64 = (ROWS * 2) as i64 - 26;

/// A panel step in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PanelView {
    step: usize,
    local: f64,
    slide: panel::Slide,
}

/// Everything the paint layers need to know about this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    logo: LogoPhase,
    /// Heading text and its drop-in progress (0..=1).
    heading: Option<(&'static str, f64)>,
    panel: Option<PanelView>,
    /// The previous panel, while the current one pushes it off screen.
    pushed: Option<PanelView>,
    /// Pixels the scroll text has scrolled in.
    scroll: f64,
    /// Seconds since the `{hills}` code (None: the landscape isn't in yet).
    hills: Option<f64>,
}

/// What is on screen at script time `t`: the scroller, plus the landscape
/// once `{hills}` and the wordmark once `{logo}` has fired.
fn script_view(tl: &script::Timeline, t: f64) -> View {
    let logo = tl.since(script::Code::Logo, t).map_or(LogoPhase::Hidden, |at| LogoPhase::Enter(t - at));
    let hills = tl.since(script::Code::Hills, t).map(|at| t - at);
    View { logo, heading: None, panel: None, pushed: None, scroll: tl.scrolled(t), hills }
}

/// What is on screen `local` seconds into storyboard step `i`. The
/// storyboard (panels, headings) is re-attached to the script by later
/// codes; until then only the tests drive it.
#[cfg_attr(not(test), allow(dead_code))]
fn view_for(steps: &[Step], i: usize, local: f64) -> View {
    let step = steps[i];
    let logo = match step.kind {
        StepKind::Intro => LogoPhase::Enter(local),
        StepKind::LogoRise => LogoPhase::Rise(local),
        StepKind::Finale => LogoPhase::Fall(local),
        _ => LogoPhase::Top,
    };
    let heading = steps[..=i]
        .iter()
        .rposition(|s| s.block == step.block && matches!(s.kind, StepKind::Heading { .. }))
        .map(|h| {
            let StepKind::Heading { title } = steps[h].kind else { unreachable!() };
            let progress = if h == i { (local / stage::HEADING_SECS).min(1.0) } else { 1.0 };
            (title, progress)
        });
    let panel = is_panel(&steps[i]).then(|| PanelView { step: i, local, slide: slide_of(steps, i) });
    let pushed = panel
        .filter(|pv| !pv.slide.first && local < panel::ENTER_SECS)
        .map(|_| PanelView { step: i - 1, local: steps[i - 1].secs + local, slide: slide_of(steps, i - 1) });
    View { logo, heading, panel, pushed, scroll: 0.0, hills: Some(f64::INFINITY) }
}

#[cfg_attr(not(test), allow(dead_code))]
fn is_panel(step: &Step) -> bool {
    matches!(step.kind, StepKind::Panel { .. })
}

/// Whether panel step `i` opens or closes its block's run of panels.
#[cfg_attr(not(test), allow(dead_code))]
fn slide_of(steps: &[Step], i: usize) -> panel::Slide {
    let same_run = |j: usize| is_panel(&steps[j]) && steps[j].block == steps[i].block;
    panel::Slide { first: i == 0 || !same_run(i - 1), last: i + 1 >= steps.len() || !same_run(i + 1) }
}

/// The panels on screen this frame — the current one and, while it slides
/// in, the one it pushes out — each as (step, interior at its current
/// place, local time).
fn on_screen(steps: &[Step], view: &View, screen_w: usize) -> Vec<(usize, panel::Geometry, f64)> {
    let Some(pv) = view.panel else { return Vec::new() };
    let step = steps[pv.step];
    let rest = panel::geometry(screen_w, step.block);
    let x = panel::slide_x(&rest, screen_w, pv.local, step.secs, pv.slide);
    let mut out = vec![(pv.step, panel::Geometry { x, ..rest }, pv.local)];
    if let Some(old) = view.pushed {
        let x = x - panel::travel(&rest, screen_w);
        out.push((old.step, panel::Geometry { x, ..rest }, old.local));
    }
    out
}

/// Plays the script: applies seeks, publishes the frame's `View` for the
/// paint layers, renders them, then blits any panel effects.
struct Stage {
    steps: Vec<Step>,
    /// Per step: the panel's effect (None for non-panel steps).
    effects: Vec<Option<Box<dyn Effect>>>,
    screen_w: usize,
    layers: Layers,
    /// The block heading, drawn last so it sits in front of the panels.
    heading: Layers,
    view: Arc<Mutex<View>>,
    /// The scroll text laid out for this screen.
    #[cfg(test)]
    glyphs: Vec<script::Glyph>,
    /// Shared with its paint layer; also puts terminal-font letters on.
    scroller: Arc<scroller::Scroller>,
    timeline: script::Timeline,
    /// Wall time at which script time was 0.
    origin: Mutex<f64>,
    seek: Arc<Mutex<Option<Seek>>>,
    /// Tests: show the storyboard from this time instead of the script.
    #[cfg(test)]
    story: Mutex<Option<f64>>,
}

impl Stage {
    fn new(screen_w: usize) -> Self {
        let steps = storyboard();
        let effects = steps
            .iter()
            .map(|st| match st.kind {
                StepKind::Panel { kind, name, .. } => {
                    let geo = panel::geometry(screen_w, st.block);
                    Some(panel::effect_for(kind, &panel::name_art(name, geo.w - 2).join("\n")))
                }
                _ => None,
            })
            .collect();
        let placed = script::parse(scroller::PITCH)
            .and_then(|s| s.place(screen_w))
            .unwrap_or_else(|e| panic!("scroll text: {e}"));
        let timeline = script::Timeline::new(&placed, screen_w);
        let view = Arc::new(Mutex::new(script_view(&timeline, 0.0)));
        let scroller = Arc::new(scroller::Scroller::new(placed.glyphs.clone(), SCROLLER_Y));
        let layers = build_layers(screen_w, view.clone(), steps.clone(), scroller.clone());
        let heading = build_heading(view.clone());
        Self {
            steps,
            effects,
            screen_w,
            layers,
            heading,
            view,
            #[cfg(test)]
            glyphs: placed.glyphs,
            scroller,
            timeline,
            origin: Mutex::new(0.0),
            seek: Arc::new(Mutex::new(None)),
            #[cfg(test)]
            story: Mutex::new(None),
        }
    }

    /// Jump the clock so script time is `t` at frame 0 (tests).
    #[cfg(test)]
    fn seek_to(&self, t: f64) {
        *self.origin.lock().unwrap() = -t;
        *self.story.lock().unwrap() = None;
    }

    /// Show the storyboard from time `t` at frame 0 instead of the script
    /// (tests of the panels, until codes drive them).
    #[cfg(test)]
    fn story_at(&self, t: f64) {
        *self.story.lock().unwrap() = Some(t);
    }

    /// Script time at wall time `now`, after applying a pending seek:
    /// → jumps to the next code, ← to the previous one; past the end the
    /// show starts over.
    fn script_time(&self, now: f64) -> f64 {
        let mut origin = self.origin.lock().unwrap();
        let t = (now - *origin).rem_euclid(self.timeline.total);
        let target = match self.seek.lock().unwrap().take() {
            Some(Seek::Next) => self.timeline.next_after(t),
            Some(Seek::Prev) => self.timeline.prev_before(t),
            _ => return t,
        };
        *origin = now - target;
        target
    }

    /// This frame's view.
    fn view_at(&self, frame: usize) -> View {
        let now = frame as f64 / FPS;
        #[cfg(test)]
        if let Some(t0) = *self.story.lock().unwrap() {
            let (i, local) = locate(&self.steps, t0 + now);
            return View { scroll: (t0 + now) * FPS, ..view_for(&self.steps, i, local) };
        }
        script_view(&self.timeline, self.script_time(now))
    }
}

impl Effect for Stage {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let view = self.view_at(frame);
        *self.view.lock().unwrap() = view;

        self.layers.render(buf, frame);
        self.scroller.paint_cells(buf, view.scroll, frame);

        for (s, geo, local) in on_screen(&self.steps, &view, self.screen_w) {
            if let Some(effect) = &self.effects[s] {
                panel::blit(buf, &geo, effect.as_ref(), panel::effect_frame(local));
            }
        }
        if let Some(pv) = view.panel {
            let step = self.steps[pv.step];
            if let StepKind::Panel { subtitle, .. } = step.kind {
                if panel::at_rest(pv.local, step.secs, pv.slide) {
                    put_band(buf, subtitle_row(&panel::geometry(self.screen_w, step.block)), subtitle);
                }
            }
        }

        // The heading goes over everything else; headings too wide for the
        // pixel font go in as plain text.
        if let Some((title, _)) = view.heading {
            if pixel_caption_fits(title, buf.width) {
                let mut top = FrameBuffer::new(buf.width, buf.height);
                self.heading.render(&mut top, frame);
                for y in 0..buf.height {
                    for x in 0..buf.width {
                        let over = top.get(x, y);
                        if over.ch != ' ' {
                            buf.set(x, y, BlendMode::Normal.composite(buf.get(x, y), over, 1.0));
                        }
                    }
                }
            } else {
                put_band(buf, (layout::Layout::for_width(buf.width).heading_y / 2) as usize, title);
            }
        }

        if buf.width > 0 && buf.height > 0 {
            frame_edges(buf);
        }
    }

    fn size(&self) -> (usize, usize) {
        (0, ROWS)
    }
}

/// Clear a one-cell border around the scene to the terminal's own
/// background. Terminals like iTerm stretch edge cells' backgrounds into
/// the window margin; this way that margin is a plain frame instead of
/// stretched scenery.
fn frame_edges(buf: &mut FrameBuffer) {
    let (w, h) = (buf.width, buf.height);
    for y in 0..h {
        buf.set(0, y, Cell::space());
        buf.set(w - 1, y, Cell::space());
    }
    for x in 0..w {
        buf.set(x, 0, Cell::space());
        buf.set(x, h - 1, Cell::space());
    }
}


/// Terminal row the subtitle sits on: the panel's bottom interior row.
fn subtitle_row(geo: &panel::Geometry) -> usize {
    (geo.y + geo.h as i64 - 1) as usize
}

/// Whether a caption fits on screen in the small pixel font.
fn pixel_caption_fits(text: &str, width: usize) -> bool {
    tiny::width(text) + 2 <= width as i64
}

/// Write `text` centred on `row` as plain terminal text — cream on a panel-
/// coloured band one space wider each side — so any length fits. Clipped
/// to the buffer.
fn put_band(buf: &mut FrameBuffer, row: usize, text: &str) {
    if row >= buf.height {
        return;
    }
    let band = format!(" {text} ");
    let x0 = (buf.width as i64 - band.chars().count() as i64) / 2;
    for (i, ch) in band.chars().enumerate() {
        let x = x0 + i as i64;
        if x >= 0 && (x as usize) < buf.width {
            buf.set(x as usize, row, Cell::with_bg(ch, glyphs::LETTER_HIGHLIGHT, panel::PANEL_FILL));
        }
    }
}

fn build_layers(screen_w: usize, view: Arc<Mutex<View>>, steps: Vec<Step>, scroller: Arc<scroller::Scroller>) -> Layers {
    let word = wordmark::Wordmark::placed(screen_w);
    // Baseline high enough that the scroller's wave plus extrusion stays on screen.

    let logo_view = view.clone();
    let scroll_view = view.clone();
    let hills_view = view.clone();
    let frame_view = view;
    let frame_steps = steps;

    landscape::landscape(move || hills_view.lock().unwrap().hills)
        .layer(Layer::paint(move |c| {
            let phase = logo_view.lock().unwrap().logo;
            word.paint(c, phase);
            word.paint_tagline(c, phase);
        }))
        // Panel boxes (fill, edge, extrusion).
        .layer(Layer::paint(move |c| {
            let view = *frame_view.lock().unwrap();
            for (_, geo, _) in on_screen(&frame_steps, &view, c.width) {
                panel::paint_frame(c, &geo);
            }
        }))
        .layer(Layer::paint(move |c| scroller.paint(c, scroll_view.lock().unwrap().scroll)))
}

/// The block heading under the panel in the small font, popping up with
/// a bounce (plain text when it is too wide — see Stage::render).
fn build_heading(view: Arc<Mutex<View>>) -> Layers {
    Layers::new(ROWS).layer(Layer::paint(move |c| {
        let Some((title, progress)) = view.lock().unwrap().heading else { return };
        let x = (c.width as i64 - tiny::width(title)) / 2;
        let y = layout::Layout::for_width(c.width).heading_y + ((1.0 - Easing::BounceOut.apply(progress)) * 12.0).round() as i64;
        tiny::stamp(c, title, (x, y), usize::MAX);
    }))
}

/// Why a terminal `rows` tall can't show the scene, if it can't.
fn size_problem(rows: usize) -> Option<String> {
    (rows < ROWS).then(|| {
        format!("hills needs a terminal at least {ROWS} rows tall (this one has {rows}) — make the window taller or zoom out.")
    })
}

#[tokio::main]
async fn main() {
    if let Some(problem) = size_problem(terminal_height()) {
        eprintln!("{problem}");
        return;
    }
    let stage = Stage::new(terminal_width());
    let seek = stage.seek.clone();
    Scene::new()
        .add(stage)
        .fullscreen()
        .quit_key('q')
        .on_key(move |key| {
            let request = match key {
                Key::Right | Key::Char('n') | Key::Char(' ') => Seek::Next,
                Key::Left | Key::Char('p') => Seek::Prev,
                _ => return,
            };
            *seek.lock().unwrap() = Some(request);
        })
        .run(24.0 * 3600.0)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use stage::{start_of, storyboard, StepKind};

    #[test]
    fn intro_and_exit_drive_the_logo() {
        let s = storyboard();
        assert_eq!(view_for(&s, 0, 2.0).logo, wordmark::LogoPhase::Enter(2.0));
        assert_eq!(view_for(&s, 1, 0.5).logo, wordmark::LogoPhase::Rise(0.5));
        assert!(view_for(&s, 0, 2.0).heading.is_none());
    }

    #[test]
    fn heading_drops_then_stays_for_its_block() {
        let s = storyboard();
        let h = s.iter().position(|st| matches!(st.kind, StepKind::Heading { .. })).unwrap();
        let (title, p) = view_for(&s, h, stage::HEADING_SECS / 2.0).heading.unwrap();
        assert_eq!((title, p), ("color effects", 0.5));
        assert_eq!(view_for(&s, h + 3, 1.0).heading, Some(("color effects", 1.0)));
        assert_eq!(view_for(&s, h + 3, 1.0).logo, wordmark::LogoPhase::Top);
    }

    #[test]
    fn panel_steps_expose_a_panel() {
        let s = storyboard();
        let p = s.iter().position(|st| matches!(st.kind, StepKind::Panel { .. })).unwrap();
        let v = view_for(&s, p, 1.0);
        assert_eq!(v.panel.map(|pv| (pv.step, pv.local)), Some((p, 1.0)));
        assert_eq!(v.pushed, None);
    }

    #[test]
    fn finale_shows_logo_only() {
        let s = storyboard();
        let v = view_for(&s, s.len() - 1, 1.0);
        assert_eq!((v.logo, v.heading, v.panel), (wordmark::LogoPhase::Fall(1.0), None, None));
    }

    #[test]
    fn next_panel_pushes_the_current_one_out() {
        let s = storyboard();
        let p = s.iter().position(is_panel).unwrap();
        assert!(slide_of(&s, p).first && !slide_of(&s, p).last);
        // The block's second panel, a moment into its slide-in.
        let v = view_for(&s, p + 1, 0.2);
        let old = v.pushed.expect("first panel still on screen");
        assert_eq!((old.step, old.local), (p, s[p].secs + 0.2));
        let on = on_screen(&s, &v, 200);
        assert_eq!(on.len(), 2);
        assert!(on[1].1.x < on[0].1.x, "outgoing panel is to the left");
        assert_eq!(on[0].1.x - on[1].1.x, panel::travel(&panel::geometry(200, 2), 200));
        assert_eq!(view_for(&s, p + 1, panel::ENTER_SECS).pushed, None);
    }

    #[test]
    fn last_panel_of_a_block_leaves_alone() {
        let s = storyboard();
        let last = s.iter().rposition(|st| st.block == 2 && is_panel(st)).unwrap();
        assert!(slide_of(&s, last).last);
        let first_of_next = s.iter().position(|st| st.block == 3 && is_panel(st)).unwrap();
        assert!(slide_of(&s, first_of_next).first);
        assert_eq!(view_for(&s, first_of_next, 0.1).pushed, None);
    }

    #[test]
    fn heading_is_drawn_over_everything() {
        // The heading goes on after the panels' cell effects and subtitles:
        // wherever it is solid, it is what you see.
        let s = storyboard();
        let p = s.iter().position(is_panel).unwrap();
        let stage = Stage::new(200);
        stage.story_at(start_of(&s, p) + 2.0);
        let mut b = FrameBuffer::new(200, ROWS);
        stage.render(&mut b, 0);
        let mut heading = FrameBuffer::new(200, ROWS);
        stage.heading.render(&mut heading, 0);
        let ink = (0..ROWS)
            .flat_map(|y| (0..200).map(move |x| (x, y)))
            .filter(|&(x, y)| heading.get(x, y).ch != ' ')
            .count();
        assert!(ink > 0);
        let mut solid = 0;
        for y in 0..ROWS {
            for x in 1..199 {
                let h = heading.get(x, y);
                // Both pixels set: opaque, so it reads exactly as the heading.
                if h.ch == '▀' && h.bg.is_some() && y > 0 {
                    solid += 1;
                    assert_eq!(b.get(x, y), BlendMode::Normal.composite(Cell::space(), h, 1.0), "({x},{y})");
                }
            }
        }
        assert!(solid > 20, "only {solid} solid heading cells");
    }

    /// `cargo test --release --example hills bench_blocks -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_blocks() {
        let s = storyboard();
        let stage = Stage::new(200);
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            stage.story_at(start_of(&s, i) + 1.0);
            let start = std::time::Instant::now();
            for f in 0..120 {
                let mut b = FrameBuffer::new(200, ROWS);
                stage.render(&mut b, f);
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / 120.0;
            println!("block {block}: {ms:.2} ms/frame");
            assert!(ms < 8.0, "block {block} too slow: {ms:.2} ms/frame");
        }
    }

    /// `cargo test --release --example hills dump_blocks -- --ignored`, then
    /// `cat target/hills-blocks.ansi` in a 120-column terminal.
    #[test]
    #[ignore]
    fn dump_blocks() {
        let s = storyboard();
        let mut out = String::new();
        for w in [112, 200] {
            let stage = Stage::new(w);
            for block in 1..=6 {
                let i = s.iter().position(|st| st.block == block).unwrap();
                let target = if block == 1 { 0 } else { i + 1 }; // first panel of the block
                let target = target.min(s.len() - 1);
                stage.story_at(start_of(&s, target) + 2.0);
                let mut b = FrameBuffer::new(w, ROWS);
                stage.render(&mut b, 0);
                out += &format!("\n=== {w} block {block} ===\n{}\n", b.to_ansi_string());
            }
            // Mid-push: the second colour panel shoving the first one out.
            let p = s.iter().position(is_panel).unwrap() + 1;
            stage.story_at(start_of(&s, p) + 0.3);
            let mut b = FrameBuffer::new(w, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== {w} push ===\n{}\n", b.to_ansi_string());
            // Intro after the tagline has typed out.
            stage.story_at(5.0);
            let mut b = FrameBuffer::new(w, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== {w} intro ===\n{}\n", b.to_ansi_string());
        }
        // The script: before {logo}, and with the logo in and the tagline typed.
        let stage = Stage::new(112);
        let cue = stage.timeline.since(script::Code::Logo, f64::MAX).unwrap();
        let hills = stage.timeline.since(script::Code::Hills, f64::MAX).unwrap();
        for (name, t) in [
            ("opening", hills - 0.3),
            ("hills 0.1", hills + 0.1),
            ("hills 0.4", hills + 0.4),
            ("hills 0.9", hills + 0.9),
            ("hills 1.3", hills + 1.3),
            ("hills 2.2", hills + 2.2),
            ("script before", cue - 1.0),
            ("script logo", cue + 4.0),
        ] {
            stage.seek_to(t);
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== {name} ===\n{}\n", b.to_ansi_string());
        }
        // One frame per scroller font/style, its first letter mid-screen.
        use scroller::{Font, Style};
        for (name, font, style) in [
            ("font tiny", Font::Tiny, Style::Candy),
            ("font big", Font::Big, Style::Candy),
            ("font term", Font::Term, Style::Candy),
            ("style flat", Font::C64, Style::Flat),
            ("font mario", Font::Mario, Style::Candy),
        ] {
            let g = stage.glyphs.iter().find(|g| g.font == font && g.style == style).unwrap();
            let target = g.x + 112.0 / 2.0 - 30.0;
            let t = (0..).map(|f| f as f64 / FPS).find(|&t| stage.timeline.scrolled(t) >= target).unwrap();
            stage.seek_to(t);
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== {name} ===\n{}\n", b.to_ansi_string());
        }
        std::fs::write("target/hills-blocks.ansi", out).unwrap();
    }

    #[test]
    fn subtitle_is_plain_text_under_the_resting_panel() {
        let s = storyboard();
        let stage = Stage::new(120);
        let p = s.iter().position(|st| matches!(st.kind, StepKind::Panel { .. })).unwrap();
        let StepKind::Panel { subtitle, .. } = s[p].kind else { unreachable!() };
        stage.story_at(start_of(&s, p) + 2.0);
        let mut b = FrameBuffer::new(120, ROWS);
        stage.render(&mut b, 0);
        let row = subtitle_row(&panel::geometry(120, s[p].block));
        let line: Vec<char> = (0..120).map(|x| b.get(x, row).ch).collect();
        let want: Vec<char> = subtitle.chars().collect();
        let x = line.windows(want.len()).position(|w| w == want.as_slice());
        let x = x.unwrap_or_else(|| panic!("row {row}: {:?}", line.iter().collect::<String>()));
        assert_eq!(b.get(x, row).bg, Some(panel::PANEL_FILL));
    }

    #[test]
    fn heading_falls_back_to_plain_text_when_too_wide() {
        let s = storyboard();
        let stage = Stage::new(50);
        let p = s.iter().position(|st| st.block == 3 && matches!(st.kind, StepKind::Panel { .. })).unwrap();
        stage.story_at(start_of(&s, p) + 2.0);
        let mut b = FrameBuffer::new(50, ROWS);
        stage.render(&mut b, 0);
        let found = (0..ROWS).any(|y| {
            let line: String = (0..50).map(|x| b.get(x, y).ch).collect();
            line.contains("shape & layout")
        });
        assert!(found, "plain heading not found");
    }

    #[test]
    fn scroller_wave_clears_the_bottom_border_with_room_to_spare() {
        let glyphs = script::parse(scroller::PITCH).unwrap().place(120).unwrap().glyphs;
        let scroller = scroller::Scroller::new(glyphs, SCROLLER_Y);
        let layers = Layers::new(ROWS).layer(Layer::paint(move |c| scroller.paint(c, c.frame as f64)));
        let mut lowest = 0;
        for frame in 0..120 {
            let mut b = FrameBuffer::new(120, ROWS);
            layers.render(&mut b, frame);
            for y in 0..ROWS {
                if (0..120).any(|x| b.get(x, y).ch != ' ') {
                    lowest = lowest.max(y);
                }
            }
        }
        // At least three free rows between the wave's lowest point and the border.
        assert!(lowest + 4 < ROWS - 1, "scroller reaches row {lowest} of {ROWS}");
    }

    #[test]
    fn logo_waits_for_its_code() {
        let stage = Stage::new(120);
        let cue = stage.timeline.since(script::Code::Logo, f64::MAX).expect("pitch has a {logo}");
        stage.seek_to(cue - 0.5);
        assert_eq!(stage.view_at(0).logo, LogoPhase::Hidden);
        stage.seek_to(cue + 1.0);
        let LogoPhase::Enter(t) = stage.view_at(0).logo else { panic!("logo not entering") };
        assert!((t - 1.0).abs() < 1e-9);
        // Before the code: landscape and scroller only.
        stage.seek_to(0.5);
        let v = stage.view_at(0);
        assert_eq!((v.logo, v.heading, v.panel, v.hills), (LogoPhase::Hidden, None, None, None));
        assert!(v.scroll > 0.0);
    }

    #[test]
    fn screen_is_empty_of_text_when_the_logo_drops() {
        for w in [80, 112, 200] {
            let stage = Stage::new(w);
            let cue = stage.timeline.since(script::Code::Logo, f64::MAX).unwrap();
            let scrolled = stage.timeline.scrolled(cue);
            for g in &stage.glyphs {
                let x = w as f64 - scrolled + g.x;
                let wide = scroller::advance(g.font, g.ch) as f64;
                assert!(x <= -wide || x >= w as f64, "{w}: {:?} on screen at x={x}", g.ch);
            }
        }
    }

    #[test]
    fn opening_line_holds_still_in_the_middle() {
        for w in [80, 112, 200] {
            let stage = Stage::new(w);
            let pause = stage.timeline.cues.iter().find(|c| matches!(c.code, script::Code::Pause(_))).unwrap();
            let script::Code::Pause(secs) = pause.code else { unreachable!() };
            assert!(secs >= 2.5, "long enough to wonder what this is");
            // The opening line: the terminal-font letters before the logo.
            let line: Vec<_> = stage.glyphs.iter().take_while(|g| g.font == scroller::Font::Term).collect();
            let scrolled = stage.timeline.scrolled(pause.t + secs / 2.0);
            let left = w as f64 - scrolled + line.first().unwrap().x;
            let right = w as f64 - scrolled + line.last().unwrap().x + 1.0;
            let centre = (left + right) / 2.0;
            assert!((centre - w as f64 / 2.0).abs() <= 2.0, "{w}: line centred at {centre}");
            // It stands there alone first — no landscape yet — and the
            // hills only start once that first pause is over.
            assert_eq!(stage.timeline.since(script::Code::Hills, pause.t + secs - 0.01), None);
            assert_eq!(stage.timeline.since(script::Code::Hills, pause.t + secs), Some(pause.t + secs));
        }
    }

    #[test]
    fn space_jumps_to_the_next_code() {
        let stage = Stage::new(120);
        stage.seek_to(0.0);
        let next = |frame| {
            *stage.seek.lock().unwrap() = Some(Seek::Next);
            stage.view_at(frame)
        };
        // First stop: the opening line pauses mid-screen, nothing else yet.
        let v = next(0);
        assert_eq!((v.hills, v.logo), (None, LogoPhase::Hidden));
        // Then the hills come in.
        let v = next(60);
        assert_eq!((v.hills, v.logo), (Some(0.0), LogoPhase::Hidden));
        // Then the logo drops — and the jump sticks: a second later it's a second in.
        assert_eq!(next(120).logo, LogoPhase::Enter(0.0));
        assert_eq!(stage.view_at(180).logo, LogoPhase::Enter(1.0));
    }

    #[test]
    fn show_starts_over_after_the_text() {
        let stage = Stage::new(120);
        stage.seek_to(stage.timeline.total + 0.5);
        let v = stage.view_at(0);
        assert_eq!(v.logo, LogoPhase::Hidden);
        assert!((v.scroll - 30.0).abs() < 1e-6, "scroll {}", v.scroll);
    }

    #[test]
    fn size_check_needs_the_scene_height() {
        assert!(size_problem(24).unwrap().contains("50"));
        assert_eq!(size_problem(ROWS), None);
        assert_eq!(size_problem(60), None);
    }

    #[test]
    fn scene_has_a_terminal_coloured_border() {
        // Terminals like iTerm stretch the edge cells' background into the
        // window margin; a border in the terminal's own background makes
        // that margin one plain frame around the scene.
        let s = storyboard();
        let stage = Stage::new(120);
        for t in [2.0, start_of(&s, 3) + 2.0] {
            stage.story_at(t);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            let edges = (0..ROWS)
                .flat_map(|y| [(0, y), (119, y)])
                .chain((0..120).flat_map(|x| [(x, 0), (x, ROWS - 1)]));
            for (x, y) in edges {
                assert_eq!(b.get(x, y), Cell::space(), "({x},{y}) at t={t}");
            }
            assert_ne!(b.get(1, ROWS - 2), Cell::space(), "scene starts inside the border");
        }
    }

    #[test]
    fn stage_renders_every_block() {
        let s = storyboard();
        let stage = Stage::new(120);
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            stage.story_at(start_of(&s, i) + 2.0);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            assert!(b.to_ansi_string().contains('▀'), "block {block}");
        }
    }
}
