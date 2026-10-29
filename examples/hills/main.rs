//! Hills showcase — every chromakopia effect, cracktro style, over a
//! landscape in the docs site's colours — plum fog, sage hills, brand
//! green (half-block pixel `Layers`).
//!
//! The show: CHROMAKOPIA drops in letter by letter and rides a DYCP wave,
//! the MIT licence starts scrolling along the bottom (and keeps going),
//! the letters jump away, and then block by block — COLOR EFFECTS, SHAPE &
//! LAYOUT, FIELDS, MIXING — each effect gets its own panel: near-black box
//! with the letters' outline and 3D extrusion, bouncing in with the
//! effect's name in figlet, a heading above and a subtitle below. A finale
//! drops the wordmark in again, and the show loops.
//!
//! Run: `cargo run --release --example hills`
//! Keys: → n space next · ← p previous · ↓ ↑ next/previous block ·
//! 1–6 jump to block · q quit

mod figlet;
mod glyphs;
mod landscape;
mod panel;
mod scroller;
mod stage;
mod wordmark;

use chromakopia::prelude::*;
use std::sync::{Arc, Mutex};

use chromakopia::animate::framebuffer::FPS;
use chromakopia::animate::Cell;
use stage::{locate, storyboard, Clock, Seek, Step, StepKind};
use wordmark::LogoPhase;

/// Scene height in terminal rows (two pixels each).
pub const ROWS: usize = 40;
/// Rows of open sky above the landscape, so the wordmark has headroom.
pub const TOP_PAD: usize = 6;

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
}

/// What is on screen `local` seconds into step `i`.
fn view_for(steps: &[Step], i: usize, local: f64) -> View {
    let step = steps[i];
    let logo = match step.kind {
        StepKind::Intro | StepKind::Finale => LogoPhase::Enter(local),
        StepKind::LogoExit => LogoPhase::Exit(local),
        _ => LogoPhase::Hidden,
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
    View { logo, heading, panel, pushed }
}

fn is_panel(step: &Step) -> bool {
    matches!(step.kind, StepKind::Panel { .. })
}

/// Whether panel step `i` opens or closes its block's run of panels.
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

/// Plays the storyboard: applies seeks, publishes the frame's `View` for
/// the paint layers, renders them, then blits the active panel effect.
struct Stage {
    steps: Vec<Step>,
    /// Per step: the panel's effect (None for non-panel steps).
    effects: Vec<Option<Box<dyn Effect>>>,
    screen_w: usize,
    layers: Layers,
    /// The block heading, drawn last so it sits in front of the panels.
    heading: Layers,
    view: Arc<Mutex<View>>,
    clock: Mutex<Clock>,
    seek: Arc<Mutex<Option<Seek>>>,
}

impl Stage {
    fn new(screen_w: usize) -> Self {
        let steps = storyboard();
        let effects = steps
            .iter()
            .map(|st| match st.kind {
                StepKind::Panel { kind, name, .. } => {
                    let geo = panel::geometry(screen_w, st.block);
                    Some(panel::effect_for(kind, &figlet::fit(name, geo.w - 2).join("\n")))
                }
                _ => None,
            })
            .collect();
        let view = Arc::new(Mutex::new(view_for(&steps, 0, 0.0)));
        let layers = build_layers(screen_w, view.clone(), steps.clone());
        let heading = build_heading(view.clone());
        Self { steps, effects, screen_w, layers, heading, view, clock: Mutex::new(Clock::new()), seek: Arc::new(Mutex::new(None)) }
    }

    /// Jump the clock so storyboard time is `t` at frame 0 (tests).
    #[cfg(test)]
    fn seek_to(&self, t: f64) {
        *self.clock.lock().unwrap() = Clock::at(t);
    }
}

impl Effect for Stage {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        let now = frame as f64 / FPS;
        let seek = self.seek.lock().unwrap().take();
        let t = self.clock.lock().unwrap().time(&self.steps, now, seek);
        let (i, local) = locate(&self.steps, t);
        let view = view_for(&self.steps, i, local);
        *self.view.lock().unwrap() = view;

        self.layers.render(buf, frame);

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
                put_band(buf, HEADING_ROW, title);
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

/// Paint the outermost columns and the top row one flat night-sky colour.
/// Terminals like iTerm stretch edge cells' backgrounds into the window
/// margin; with half-block dithering that shows as stripes.
fn frame_edges(buf: &mut FrameBuffer) {
    let edge = Cell::with_bg(' ', landscape::SKY, landscape::SKY);
    for y in 0..buf.height {
        buf.set(0, y, edge);
        buf.set(buf.width - 1, y, edge);
    }
    for x in 0..buf.width {
        buf.set(x, 0, edge);
    }
}

/// Terminal row a plain-text heading sits on: where the pixel one would.
const HEADING_ROW: usize = 4;
/// Top pixel row of the resting pixel-font heading.
const HEADING_Y: i64 = HEADING_ROW as i64 * 2;

/// Terminal row the subtitle sits on: just below the panel's extrusion.
fn subtitle_row(geo: &panel::Geometry) -> usize {
    (geo.y + geo.h as i64 + 2) as usize
}

/// Whether a caption fits on screen in the 8-column pixel font.
fn pixel_caption_fits(text: &str, width: usize) -> bool {
    glyphs::text_width(text) + 2 <= width as i64
}

/// Write `text` centred on `row` as plain terminal text — glow on a panel-
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

fn build_layers(screen_w: usize, view: Arc<Mutex<View>>, steps: Vec<Step>) -> Layers {
    let (word, x0, y0) = wordmark::Wordmark::placed(screen_w);
    // Baseline high enough that the scroller's wave plus extrusion stays on screen.
    let scroller = scroller::Scroller::new(scroller::LICENSE_TEXT, (ROWS * 2) as i64 - 15);

    let logo_view = view.clone();
    let frame_view = view;
    let frame_steps = steps;

    landscape::landscape()
        .layer(Layer::paint(move |c| {
            let phase = logo_view.lock().unwrap().logo;
            word.paint(c, x0, y0, phase)
        }))
        // Panel boxes (fill, edge, extrusion).
        .layer(Layer::paint(move |c| {
            let view = *frame_view.lock().unwrap();
            for (_, geo, _) in on_screen(&frame_steps, &view, c.width) {
                panel::paint_frame(c, &geo);
            }
        }))
        .layer(Layer::paint(move |c| scroller.paint(c)))
}

/// The block heading, dropping in with a bounce (plain text when it is
/// too wide — see Stage::render).
fn build_heading(view: Arc<Mutex<View>>) -> Layers {
    Layers::new(ROWS).layer(Layer::paint(move |c| {
        let Some((title, progress)) = view.lock().unwrap().heading else { return };
        let x = (c.width as i64 - glyphs::text_width(title)) / 2;
        let y = HEADING_Y - ((1.0 - Easing::BounceOut.apply(progress)) * 20.0).round() as i64;
        glyphs::stamp_text(c, title, (x, y), &mut Vec::new());
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
                Key::Down => Seek::NextBlock,
                Key::Up => Seek::PrevBlock,
                Key::Char(c @ '1'..='6') => Seek::Block(c as usize - '0' as usize),
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
        assert_eq!(view_for(&s, 1, 0.5).logo, wordmark::LogoPhase::Exit(0.5));
        assert!(view_for(&s, 0, 2.0).heading.is_none());
    }

    #[test]
    fn heading_drops_then_stays_for_its_block() {
        let s = storyboard();
        let h = s.iter().position(|st| matches!(st.kind, StepKind::Heading { .. })).unwrap();
        let (title, p) = view_for(&s, h, stage::HEADING_SECS / 2.0).heading.unwrap();
        assert_eq!((title, p), ("COLOR EFFECTS", 0.5));
        assert_eq!(view_for(&s, h + 3, 1.0).heading, Some(("COLOR EFFECTS", 1.0)));
        assert_eq!(view_for(&s, h + 3, 1.0).logo, wordmark::LogoPhase::Hidden);
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
        assert_eq!((v.logo, v.heading, v.panel), (wordmark::LogoPhase::Enter(1.0), None, None));
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
        stage.seek_to(start_of(&s, p) + 2.0);
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
            stage.seek_to(start_of(&s, i) + 1.0);
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
        let stage = Stage::new(120);
        let mut out = String::new();
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            let target = if block == 1 { 0 } else { i + 1 }; // first panel of the block
            let target = target.min(s.len() - 1);
            stage.seek_to(start_of(&s, target) + 2.0);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== block {block} ===\n{}\n", b.to_ansi_string());
        }
        // Mid-push: the second colour panel shoving the first one out.
        let p = s.iter().position(is_panel).unwrap() + 1;
        stage.seek_to(start_of(&s, p) + 0.3);
        let mut b = FrameBuffer::new(120, ROWS);
        stage.render(&mut b, 0);
        out += &format!("\n=== push ===\n{}\n", b.to_ansi_string());
        std::fs::write("target/hills-blocks.ansi", out).unwrap();
    }

    #[test]
    fn subtitle_is_plain_text_under_the_resting_panel() {
        let s = storyboard();
        let stage = Stage::new(120);
        let p = s.iter().position(|st| matches!(st.kind, StepKind::Panel { .. })).unwrap();
        let StepKind::Panel { subtitle, .. } = s[p].kind else { unreachable!() };
        stage.seek_to(start_of(&s, p) + 2.0);
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
        let stage = Stage::new(80);
        let p = s.iter().position(|st| st.block == 3 && matches!(st.kind, StepKind::Panel { .. })).unwrap();
        stage.seek_to(start_of(&s, p) + 2.0);
        let mut b = FrameBuffer::new(80, ROWS);
        stage.render(&mut b, 0);
        let found = (0..ROWS).any(|y| {
            let line: String = (0..80).map(|x| b.get(x, y).ch).collect();
            line.contains("SHAPE & LAYOUT")
        });
        assert!(found, "plain heading not found");
    }

    #[test]
    fn size_check_needs_the_scene_height() {
        assert!(size_problem(24).unwrap().contains("40"));
        assert_eq!(size_problem(ROWS), None);
        assert_eq!(size_problem(60), None);
    }

    #[test]
    fn edge_cells_are_one_flat_colour() {
        // Terminals like iTerm stretch the edge cells' background into the
        // window margin; anything but one flat colour there shows as stripes.
        let s = storyboard();
        let stage = Stage::new(120);
        for t in [2.0, start_of(&s, 3) + 2.0] {
            stage.seek_to(t);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            let edges = (0..ROWS).flat_map(|y| [(0, y), (119, y)]).chain((0..120).map(|x| (x, 0)));
            for (x, y) in edges {
                let cell = b.get(x, y);
                assert_eq!((cell.ch, cell.bg), (' ', Some(landscape::SKY)), "({x},{y}) at t={t}");
            }
        }
    }

    #[test]
    fn stage_renders_every_block() {
        let s = storyboard();
        let stage = Stage::new(120);
        for block in 1..=6 {
            let i = s.iter().position(|st| st.block == block).unwrap();
            stage.seek_to(start_of(&s, i) + 2.0);
            let mut b = FrameBuffer::new(120, ROWS);
            stage.render(&mut b, 0);
            assert!(b.to_ansi_string().contains('▀'), "block {block}");
        }
    }
}
