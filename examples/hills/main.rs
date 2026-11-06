//! Hills showcase — chromakopia, cracktro style, over a pastel landscape
//! of swaying dithered hills (half-block pixel `Layers`).
//!
//! The scroll text along the bottom runs the show: control codes in it
//! (see `script.rs`) fire as they pass the middle of the screen. It opens
//! on the landscape and the scroller alone; `{logo}` drops CHROMAKOPIA in
//! and types the tagline under it. The effect panels (`stage.rs`,
//! `panel.rs`) wait for codes of their own.
//!
//! Run: `cargo run --release --example hills` (`-- --mute` for no music)
//! Keys: → n space next code · ← previous code · p pause · s sequencer ·
//! 1–8 mute a track · q quit

mod dive;
mod drums;
mod figlet;
mod glyphs;
mod intro;
mod landscape;
mod layout;
mod looping;
mod music;
mod orbital3;
mod reverb;
mod panel;
mod script;
mod scroller;
mod sides;
// The storyboard's clock and seeking are idle until script codes drive
// the panels again.
#[allow(dead_code)]
mod stage;
mod tiny;
mod wordmark;

use chromakopia::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
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
    /// Seconds into the cold open, while it plays.
    intro: Option<f64>,
    /// How far into the big C the camera has dived.
    dive: dive::Dive,
    /// Per side scroller: seconds since its latest `{side}` code.
    sides: [Option<f64>; sides::SIDES.len()],
    /// Script time.
    secs: f64,
}

/// What is on screen at script time `t`: the scroller, plus the landscape
/// once `{hills}` and the wordmark once `{logo}` has fired.
fn script_view(tl: &script::Timeline, intro_secs: f64, t: f64) -> View {
    // In with `{logo}`, aside with `{logo away}`, off with `{logo out}` —
    // whichever came last. Back from aside, the big C shrinks home; back
    // from off, the logo drops in afresh.
    let away = |gone: f64| if gone < wordmark::EXIT_DONE { LogoPhase::Exit(gone) } else { LogoPhase::Hidden };
    let at = |code| tl.since(code, t).unwrap_or(f64::NEG_INFINITY);
    let (came, aside, out) = (at(script::Code::Logo), at(script::Code::LogoAway), at(script::Code::LogoOut));
    let mut logo = if came.max(aside).max(out) == f64::NEG_INFINITY {
        LogoPhase::Hidden
    } else if out > came.max(aside) {
        away(t - out)
    } else if aside > came {
        LogoPhase::Away(t - aside)
    } else if aside > out {
        LogoPhase::Return(t - came)
    } else {
        LogoPhase::Enter(t - came)
    };
    let mut hills = tl.since(script::Code::Hills, t).map(|at| t - at);
    // After `{bye}`: the logo jumps away, the landscape plays its
    // entrance backwards from wherever it had got to.
    if let Some(bye) = tl.since(script::Code::Bye, t) {
        let gone = t - bye;
        if matches!(logo, LogoPhase::Enter(_) | LogoPhase::Return(_)) {
            logo = away(gone);
        }
        hills = hills
            .map(|h| (h - gone).min(landscape::ENTRANCE_SECS) - gone * landscape::EXIT_SPEED)
            .filter(|&h| h > 0.0);
    }
    // From `{dive in}`: the zoom, then — once whatever rides the turning
    // liquid is through — the waves spread. From `{dive out}`: the waves
    // merge, then the zoom out.
    let (dive_in, dive_out) = (tl.since(script::Code::Dive(true), t), tl.since(script::Code::Dive(false), t));
    let part = |from: f64, secs: f64| ((t - from) / secs).clamp(0.0, 1.0);
    let dive = match (dive_in, dive_out) {
        (Some(a), b) if b.is_none_or(|b| a > b) => {
            let spread = tl.since(script::Code::Spread, t).filter(|&s| s >= a);
            dive::Dive { zoom: part(a, dive::DIVE_SECS), waves: spread.map_or(0.0, |s| part(s, dive::WAVES_SECS)) }
        }
        (_, Some(b)) => dive::Dive {
            zoom: 1.0 - part(b + dive::WAVES_SECS, dive::DIVE_SECS),
            waves: 1.0 - part(b, dive::WAVES_SECS),
        },
        _ => dive::Dive::default(),
    };
    let intro = (t < intro_secs).then_some(t);
    let sides = std::array::from_fn(|i| tl.since(script::Code::Side(i), t).map(|at| t - at));
    View { logo, heading: None, panel: None, pushed: None, scroll: tl.scrolled(t), hills, intro, dive, sides, secs: t }
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
    View { logo, heading, panel, pushed, scroll: 0.0, hills: Some(f64::INFINITY), intro: None, dive: dive::Dive::default(), sides: [None; sides::SIDES.len()], secs: 0.0 }
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
    /// The cold open, played before the scroller starts.
    intro: intro::Intro,
    timeline: script::Timeline,
    /// Wall time at which script time was 0.
    origin: Mutex<f64>,
    seek: Arc<Mutex<Option<Seek>>>,
    /// The music's loops, for the sequencer strip.
    session: music::Session,
    /// Whether the sequencer strip shows: hidden until `s` toggles it.
    sequencer: Arc<AtomicBool>,
    /// Tracks muted from the keyboard (1–8), shared with the music.
    mutes: music::Mutes,
    /// Paused (`p`), shared with the music.
    paused: Arc<AtomicBool>,
    /// (last frame seen, frames spent paused): the show's own frame count
    /// stands still in a pause, so it goes on from there seamlessly.
    held: Mutex<(usize, usize)>,
    /// The script time, as f64 bits, each frame: the music follows it.
    clock: Arc<AtomicU64>,
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
        let cold_open = intro::Intro::new(screen_w, ROWS, SCROLLER_Y);
        let timeline = script::Timeline::new(&placed, screen_w).with_lead(cold_open.secs, &cold_open.marks);
        let session = music::Session::new(&timeline);
        let view = Arc::new(Mutex::new(script_view(&timeline, cold_open.shown, 0.0)));
        let scroller = Arc::new(scroller::Scroller::new(placed.glyphs.clone(), SCROLLER_Y));
        let ride = looping::ride(&placed.glyphs, screen_w);
        let layers = build_layers(screen_w, view.clone(), steps.clone(), scroller.clone(), ride);
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
            intro: cold_open,
            timeline,
            origin: Mutex::new(0.0),
            seek: Arc::new(Mutex::new(None)),
            session,
            sequencer: Arc::new(AtomicBool::new(false)),
            mutes: Arc::new(AtomicU16::new(0)),
            paused: Arc::new(AtomicBool::new(false)),
            held: Mutex::new((0, 0)),
            clock: Arc::new(AtomicU64::new(0f64.to_bits())),
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
        let t = self.script_time(now);
        self.clock.store(t.to_bits(), Ordering::Relaxed);
        script_view(&self.timeline, self.intro.shown, t)
    }
}

impl Effect for Stage {
    fn render(&self, buf: &mut FrameBuffer, frame: usize) {
        // The show's frame: the renderer's, less the frames spent paused.
        let frame = {
            let mut held = self.held.lock().unwrap();
            if self.paused.load(Ordering::Relaxed) {
                held.1 += frame.saturating_sub(held.0);
            }
            held.0 = frame;
            frame - held.1
        };
        let view = self.view_at(frame);
        *self.view.lock().unwrap() = view;

        self.layers.render(buf, frame);
        self.scroller.paint_cells(buf, view.scroll, frame);
        if let Some(t) = view.intro {
            let mut open = FrameBuffer::new(buf.width, buf.height);
            self.intro.timeline.render(&mut open, (t * FPS).round() as usize);
            for y in 0..buf.height {
                for x in 0..buf.width {
                    let cell = open.get(x, y);
                    if cell.ch != ' ' {
                        buf.set(x, y, cell);
                    }
                }
            }
        }

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

        if self.sequencer.load(Ordering::Relaxed) {
            draw_strip(buf, &self.session.strip(view.secs, self.mutes.load(Ordering::Relaxed)), view.secs);
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


/// The sequencer strip on the bottom row inside the border, as Live's
/// session view in one line: the transport (bar.beat.16th), then each
/// track — ▶ and its loop with a bar of how far through it is, a blinking
/// ◆ while a launch waits for its bar, a dim · when it's quiet — and the
/// 303s' cutoff knob. Cut off at the screen's edge.
fn draw_strip(buf: &mut FrameBuffer, strip: &music::Strip, secs: f64) {
    if buf.height < 3 || buf.width < 4 {
        return;
    }
    let row = buf.height - 2;
    let (bg, bright, dim, wait) = (glyphs::LETTER_OUTLINE, glyphs::LETTER_HIGHLIGHT, glyphs::LETTER_LOWER, glyphs::LETTER_UPPER);
    let mut parts: Vec<(String, Color)> = Vec::new();
    let (bar, beat, sixteenth) = strip.position;
    parts.push((format!(" ▶ {bar}.{beat}.{sixteenth} "), bright));
    let blink = (secs * 4.0).floor() as i64 % 2 == 0;
    for (key, t) in strip.tracks.iter().enumerate() {
        parts.push(("│ ".into(), dim));
        let key = key + 1;
        match &t.playing {
            _ if t.muted => parts.push((format!("{key}✕ {} ", t.name), dim)),
            Some((name, through)) => {
                let filled = (through * 4.0).floor() as usize;
                let bar: String = (0..4).map(|i| if i <= filled { '▰' } else { '▱' }).collect();
                let knob = t.cutoff.map(|c| format!(" c{c:.0}")).unwrap_or_default();
                parts.push((format!("{key}▶ {} {name}{knob} {bar} ", t.name), bright));
            }
            None => parts.push((format!("{key}· {} ", t.name), dim)),
        }
        if t.waiting {
            parts.push((if blink { "◆ " } else { "  " }.into(), wait));
        }
    }
    let mut x = 1;
    for (text, color) in parts {
        for ch in text.chars() {
            if x + 1 >= buf.width {
                break;
            }
            buf.set(x, row, Cell::with_bg(ch, color, bg));
            x += 1;
        }
    }
    // The rest of the row the strip's colour, edge to edge.
    for x in x..buf.width - 1 {
        buf.set(x, row, Cell::with_bg(' ', bright, bg));
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

fn build_layers(
    screen_w: usize,
    view: Arc<Mutex<View>>,
    steps: Vec<Step>,
    scroller: Arc<scroller::Scroller>,
    ride: Option<(f64, f64)>,
) -> Layers {
    let word = wordmark::Wordmark::placed(screen_w);
    // Baseline high enough that the scroller's wave plus extrusion stays on screen.

    let logo_view = view.clone();
    let scroll_view = view.clone();
    let hills_view = view.clone();
    let side_view = view.clone();
    let frame_view = view;
    let frame_steps = steps;

    let mut layers = landscape::landscape_layers(move || hills_view.lock().unwrap().hills);
    layers.push((
        sides::z::LOGO,
        Layer::paint(move |c| {
            let view = *logo_view.lock().unwrap();
            word.paint(c, view.logo, looping::tilt(view.scroll, ride), view.dive);
            let phase = view.logo;
            word.paint_tagline(c, phase);
        }),
    ));
    // Panel boxes (fill, edge, extrusion).
    layers.push((
        sides::z::PANELS,
        Layer::paint(move |c| {
            let view = *frame_view.lock().unwrap();
            for (_, geo, _) in on_screen(&frame_steps, &view, c.width) {
                panel::paint_frame(c, &geo);
            }
        }),
    ));
    layers.push((
        sides::z::SCROLLER,
        Layer::paint(move |c| {
            let view = *scroll_view.lock().unwrap();
            scroller.paint_dived(c, view.scroll, view.dive.zoom)
        }),
    ));
    // Side scrollers, each at its own depth and see-through as it says.
    for (i, side) in sides::SIDES.into_iter().enumerate() {
        let view = side_view.clone();
        layers.push((
            side.z,
            Layer::paint(move |c| {
                if let Some(secs) = view.lock().unwrap().sides[i] {
                    side.paint(c, secs);
                }
            })
            .opacity(side.opacity),
        ));
    }
    stack(layers)
}

/// `layers` back to front by depth; equal depths keep their order.
fn stack(mut layers: Vec<(i32, Layer)>) -> Layers {
    layers.sort_by_key(|&(z, _)| z);
    layers.into_iter().fold(Layers::new(ROWS), |all, (_, layer)| all.layer(layer))
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
    if std::env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal") {
        eprintln!(
            "tip: Terminal.app draws the pixels with seams and smears.\n     This looks its best in iTerm2, Ghostty, kitty or WezTerm."
        );
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    let stage = Stage::new(terminal_width());
    // The music plays for as long as this lives, the scroll text
    // launching its loops; silent without a sound device, or with --mute.
    let session = stage.session.clone();
    let _music = if std::env::args().any(|a| a == "--mute") { None } else { music::play(session, stage.clock.clone(), stage.mutes.clone(), stage.paused.clone()) };
    let seek = stage.seek.clone();
    let sequencer = stage.sequencer.clone();
    let mutes = stage.mutes.clone();
    let paused = stage.paused.clone();
    let strip_tracks = music::strip_tracks();
    Scene::new()
        .add(stage)
        .fullscreen()
        .quit_key('q')
        .on_key(move |key| {
            if key == Key::Char('s') {
                sequencer.fetch_xor(true, Ordering::Relaxed);
                return;
            }
            if key == Key::Char('p') {
                paused.fetch_xor(true, Ordering::Relaxed);
                return;
            }
            // 1–8: mute or unmute the strip's tracks.
            if let Key::Char(c @ '1'..='9') = key {
                if let Some(&track) = strip_tracks.get(c as usize - '1' as usize) {
                    mutes.fetch_xor(1 << track, Ordering::Relaxed);
                }
                return;
            }
            let request = match key {
                Key::Right | Key::Char('n') | Key::Char(' ') => Seek::Next,
                Key::Left => Seek::Prev,
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

    /// Worst stretches of the whole script, a second at a time:
    /// `cargo test --release --example hills bench_script -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_script() {
        let stage = Stage::new(200);
        let mut worst: Vec<(f64, f64)> = Vec::new();
        let mut t = 0.0;
        while t < stage.timeline.total {
            stage.seek_to(t);
            let start = std::time::Instant::now();
            for f in 0..15 {
                let mut b = FrameBuffer::new(200, ROWS);
                stage.render(&mut b, f * 4);
            }
            worst.push((start.elapsed().as_secs_f64() * 1000.0 / 15.0, t));
            t += 1.0;
        }
        let mean = worst.iter().map(|w| w.0).sum::<f64>() / worst.len() as f64;
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("mean {mean:.2} ms/frame over {} s", worst.len());
        for (ms, t) in worst.iter().take(12) {
            println!("t={t:>5.0}s scroll={:>6.0}: {ms:.2} ms/frame", stage.timeline.scrolled(*t));
        }
    }

    /// Bytes per frame the terminal has to swallow (same diff as the renderer):
    /// `cargo test --release --example hills bench_bytes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_bytes() {
        fn bytes(prev: &FrameBuffer, curr: &FrameBuffer) -> (usize, usize) {
            let (mut n, mut cells) = (0, 0);
            let (mut fg, mut bg) = (None, None);
            let digits = |c: Color| format!("{};{};{}", c.r, c.g, c.b).len();
            for y in 0..curr.height {
                let mut gap: Option<usize> = None;
                for x in 0..curr.width {
                    let c = curr.get(x, y);
                    if prev.get(x, y) == c {
                        gap = gap.map(|g| g + 1);
                        continue;
                    }
                    cells += 1;
                    n += match gap {
                        None => format!("\x1B[{};{}H", y + 1, x + 1).len(),
                        Some(0) => 0,
                        Some(g) => format!("\x1B[{g}C").len(),
                    };
                    gap = Some(0);
                    let f = (c.ch != ' ' && fg != Some(c.color)).then_some(c.color);
                    let b = (c.ch != '\u{2588}' && bg != Some(c.bg)).then_some(c.bg);
                    if f.is_some() || b.is_some() {
                        n += 3;
                    }
                    if let Some(f) = f {
                        n += 5 + digits(f);
                        fg = Some(f);
                    }
                    if let Some(b) = b {
                        n += b.map_or(2, |b| 5 + digits(b)) + usize::from(f.is_some());
                        bg = Some(b);
                    }
                    n += c.ch.len_utf8();
                }
            }
            (n, cells)
        }
        let (w, h) = (200, ROWS);
        let stage = Stage::new(w);
        let mut rows: Vec<(usize, usize, f64)> = Vec::new();
        let mut t = 0.0;
        while t < stage.timeline.total {
            stage.seek_to(t);
            let mut prev = FrameBuffer::new(w, h);
            stage.render(&mut prev, 0);
            let (mut n, mut cells) = (0, 0);
            for f in 1..=30 {
                let mut b = FrameBuffer::new(w, h);
                stage.render(&mut b, f);
                let (bn, bc) = bytes(&prev, &b);
                n += bn;
                cells += bc;
                prev = b;
            }
            rows.push((n / 30, cells / 30, t));
            t += 2.0;
        }
        let mean = rows.iter().map(|r| r.0).sum::<usize>() / rows.len();
        println!("mean {mean} bytes/frame ({} cells on screen)", w * h);
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        for (n, cells, t) in rows.iter().take(15) {
            let sc = stage.timeline.scrolled(*t);
            let text: String = stage.glyphs.iter().filter(|g| (sc - w as f64..sc).contains(&g.x)).map(|g| g.ch).take(30).collect();
            println!("t={t:>5.0}s scroll={sc:>6.0}: {n:>6} bytes/frame, {cells:>5} cells changed  {text:?}");
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
        let cue = first_cue(&stage, script::Code::Logo);
        let hills = stage.timeline.since(script::Code::Hills, f64::MAX).unwrap();
        for (name, t) in [
            ("step1", 1.0),
            ("step2", 4.5),
            ("step3", 11.0),
            ("line", stage.intro.secs + 2.0),
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
        // The spin: looping round the C, zooming in still turning, the
        // riders on the turning surface, slowing down, the waves spreading.
        let ride = looping::ride(&stage.glyphs, 112).unwrap();
        let dive_in = first_cue(&stage, script::Code::Dive(true));
        let spread = first_cue(&stage, script::Code::Spread);
        let first_in = when_scrolled(&stage, ride.0);
        for (name, t) in [
            ("a loop", first_in + 2.0),
            ("b loop", first_in + 5.0),
            ("c zoom", dive_in + 0.7),
            ("d zoomed", dive_in + 1.6),
            ("e ride", dive_in + 2.6),
            ("f ride", dive_in + 3.8),
            ("g ride", dive_in + 5.2),
            ("h slowing", spread - 2.5),
            ("i slowing", spread - 0.8),
            ("j landed", spread),
            ("k spread", spread + 0.5),
            ("l waves", spread + 1.6),
        ] {
            stage.seek_to(t);
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== spin {name} ===\n{}\n", b.to_ansi_string());
        }
        // The zoom: its first letter mid-screen.
        let first = stage.glyphs.iter().find(|g| g.zoom).unwrap();
        for (name, k) in [("zoom", 0.0), ("zoom later", 20.0)] {
            stage.seek_to(when_scrolled(&stage, first.x + 112.0 / 2.0 + 60.0 + k));
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, 0);
            out += &format!("\n=== {name} ===\n{}\n", b.to_ansi_string());
        }
        // The dive into the big C: on the way in, in, letters on the water, on the way out.
        let dive_in = first_cue(&stage, script::Code::Dive(true));
        let dive_out = first_cue(&stage, script::Code::Dive(false));
        for (name, t) in [
            ("dive 0.6", dive_in + 0.6),
            ("dive in", dive_in + dive::IN_SECS),
            ("dive ride", dive_in + dive::IN_SECS + 2.0),
            ("dive merge", dive_out + 0.4),
            ("dive out", dive_out + dive::WAVES_SECS + 0.8),
            ("dive after", dive_out + dive::OUT_SECS + 0.5),
        ] {
            stage.seek_to(t);
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, 30);
            out += &format!("\n=== {name} ===\n{}\n", b.to_ansi_string());
        }
        // The long wave over the greetings, a few seconds apart.
        let first = stage.glyphs.iter().find(|g| matches!(g.wave, script::Wave::Long(_))).unwrap();
        stage.seek_to(when_scrolled(&stage, first.x + 400.0));
        for frame in [120, 240, 360] {
            let mut b = FrameBuffer::new(112, ROWS);
            stage.render(&mut b, frame);
            out += &format!("\n=== long {frame} ===\n{}\n", b.to_ansi_string());
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
    fn side_scrollers_go_behind_the_hills_their_depth_puts_in_front() {
        let w = 120;
        let side = sides::SIDES[sides::named("hide").unwrap()];
        let secs = side.secs(w) / 2.0;
        // Cells the side scroller's letters change at depth `z`.
        let shows = |z: i32| {
            let mut layers = landscape::landscape_layers(|| Some(f64::INFINITY));
            layers.push((z, Layer::paint(move |c| side.paint(c, secs))));
            let mut with = FrameBuffer::new(w, ROWS);
            stack(layers).render(&mut with, 0);
            let mut without = FrameBuffer::new(w, ROWS);
            landscape::landscape(|| Some(f64::INFINITY)).render(&mut without, 0);
            (0..ROWS).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| with.get(x, y) != without.get(x, y)).count()
        };
        let (front, hidden) = (shows(sides::z::SCROLLER), shows(side.z));
        assert!(hidden > 0, "not there at all");
        assert!(hidden * 10 < front * 9, "the hills hide too little of it: {hidden} of {front} cells");
    }

    /// Pictures of each side scroller on its way across, one pixel per
    /// half-block: `cargo test --release --example hills dump_sides --
    /// --ignored`, then open `target/hills-sides/*.ppm`.
    #[test]
    #[ignore]
    fn dump_sides() {
        let w = 160;
        let stage = Stage::new(w);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/hills-sides");
        std::fs::create_dir_all(&dir).unwrap();
        for (i, side) in sides::SIDES.iter().enumerate() {
            for (k, part) in [0.3, 0.45, 0.55, 0.7].iter().enumerate() {
                stage.seek_to(first_cue(&stage, script::Code::Side(i)) + side.secs(w) * part);
                let mut b = FrameBuffer::new(w, ROWS);
                stage.render(&mut b, 0);
                let mut out = format!("P6 {w} {} 255\n", ROWS * 2).into_bytes();
                for py in 0..ROWS * 2 {
                    for x in 0..w {
                        let c = b.get(x, py / 2);
                        let bg = c.bg.unwrap_or(Color::new(0, 0, 0));
                        let px = match (c.ch, py % 2 == 0) {
                            ('▀', true) | ('▄', false) | ('█', _) => c.color,
                            _ => bg,
                        };
                        out.extend([px.r, px.g, px.b]);
                    }
                }
                std::fs::write(dir.join(format!("{}-{k}.ppm", side.name)), out).unwrap();
            }
        }
    }

    #[test]
    fn pitch_sends_every_side_scroller_across() {
        let w = 160;
        let stage = Stage::new(w);
        for (i, side) in sides::SIDES.iter().enumerate() {
            let half = side.secs(w) / 2.0;
            stage.seek_to(first_cue(&stage, script::Code::Side(i)) + half);
            let since = stage.view_at(0).sides[i];
            assert!(since.is_some_and(|s| (s - half).abs() < 1e-6), "{}: {since:?}", side.name);
        }
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

    /// When `code` first fires.
    fn first_cue(stage: &Stage, code: script::Code) -> f64 {
        stage.timeline.cues.iter().find(|c| c.code == code).expect("pitch has the code").t
    }

    /// When the scroll has reached `px`.
    fn when_scrolled(stage: &Stage, px: f64) -> f64 {
        (0..).map(|f| f as f64 / FPS).find(|&t| stage.timeline.scrolled(t) >= px).unwrap()
    }

    #[test]
    fn the_303s_first_loop_comes_in_as_the_text_starts_looping() {
        let first = music::clip_named("303", None).unwrap();
        for w in [100, 160, 260] {
            let stage = Stage::new(w);
            let session = music::Session::new(&stage.timeline);
            let start = when_scrolled(&stage, looping::ride(&stage.glyphs, w).unwrap().0);
            // Not before (another 303 loop may play from the logo); within
            // a bar or so of the loop starting.
            assert_ne!(session.loop_at(start - 2.5, "303"), Some(first), "w {w}: before the loop");
            assert_eq!(session.loop_at(start + 2.5, "303"), Some(first), "w {w}: not in the loop");
        }
    }

    #[test]
    fn pads_alone_then_the_808_on_welly_well_then_the_bass_with_the_colour() {
        for w in [100, 160, 260] {
            let stage = Stage::new(w);
            let session = music::Session::new(&stage.timeline);
            // The pad alone through the cold open.
            assert!(session.plays(0.0, "pad") && !session.plays(intro::BEAT_AT - 0.01, "drums"));
            // The 808 exactly on the fifth swell: "Welly well" in the middle.
            assert!(session.plays(intro::BEAT_AT, "drums"), "w {w}");
            // The bass not yet; it comes with the coloured text, by the logo.
            assert!(!session.plays(intro::BEAT_AT + 0.5, "bass"), "w {w}");
            let logo = first_cue(&stage, script::Code::Logo);
            assert!(session.plays(logo + 2.0, "bass"), "w {w}");
        }
    }

    /// Pictures of the cold open's last lines and the hand-over to the
    /// scroller, one pixel per half-block: `cargo test --release --example
    /// hills dump_intro -- --ignored`, then open `target/hills-intro/*.ppm`.
    #[test]
    #[ignore]
    fn dump_intro() {
        let w = 160;
        let stage = Stage::new(w);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/hills-intro");
        std::fs::create_dir_all(&dir).unwrap();
        for t in [9.8, 13.0, 14.5, 16.5, 18.0] {
            stage.seek_to(t);
            let mut b = FrameBuffer::new(w, ROWS);
            stage.render(&mut b, 0);
            let mut out = format!("P6 {w} {} 255\n", ROWS * 2).into_bytes();
            for py in 0..ROWS * 2 {
                for x in 0..w {
                    let c = b.get(x, py / 2);
                    let bg = c.bg.unwrap_or(Color::new(0, 0, 0));
                    let px = match (c.ch, py % 2 == 0) {
                        ('▀', true) | ('▄', false) | ('█', _) => c.color,
                        (' ', _) | ('▀', false) | ('▄', true) => bg,
                        _ => c.color,
                    };
                    out.extend([px.r, px.g, px.b]);
                }
            }
            std::fs::write(dir.join(format!("{t:04.1}.ppm")), out).unwrap();
        }
    }

    #[test]
    fn the_dycp_line_is_on_screen_on_its_swell() {
        let stage = Stage::new(160);
        stage.seek_to(intro::STEP4_START + 2.0);
        let mut buf = FrameBuffer::new(160, ROWS);
        stage.render(&mut buf, 0);
        let letters = (0..ROWS).flat_map(|y| (0..160).map(move |x| (x, y))).filter(|&(x, y)| buf.get(x, y).ch.is_ascii_alphabetic()).count();
        assert!(letters > 40, "only {letters} letters");
    }

    #[test]
    fn the_sequencer_strip_is_hidden_until_s_shows_it_at_the_bottom() {
        let stage = Stage::new(160);
        let row_text = |t: f64| {
            stage.seek_to(t);
            let mut buf = FrameBuffer::new(160, ROWS);
            stage.render(&mut buf, 0);
            (0..160).map(|x| buf.get(x, ROWS - 2).ch).collect::<String>()
        };
        // Hidden to start with: just the landscape's ground.
        assert!(!row_text(5.0).contains("pad"));
        // Shown (s): the pad plays in the cold open; the drums once Welly well's in.
        stage.sequencer.store(true, Ordering::Relaxed);
        let early = row_text(5.0);
        assert!(early.contains("1▶ pad 1") && early.contains("4· drums"), "{early}");
        let beat = row_text(intro::BEAT_AT + 0.5);
        assert!(beat.contains("4▶ drums little"), "{beat}");
        // Muted (key 4): marked, still in its place.
        stage.mutes.store(1 << music::track_named("drums").unwrap(), Ordering::Relaxed);
        assert!(row_text(intro::BEAT_AT + 0.5).contains("4✕ drums"));
        stage.mutes.store(0, Ordering::Relaxed);
        assert!(beat.contains("▶ 9."), "the transport: {beat}");
        // Toggled off: the landscape's ground again.
        stage.sequencer.store(false, Ordering::Relaxed);
        assert!(!row_text(5.0).contains("pad"));
    }

    #[test]
    fn a_pause_stands_the_show_still_and_it_goes_on_from_there() {
        let stage = Stage::new(160);
        stage.seek_to(30.0);
        let frame = |f: usize| {
            let mut buf = FrameBuffer::new(160, ROWS);
            stage.render(&mut buf, f);
            (buf, f64::from_bits(stage.clock.load(Ordering::Relaxed)))
        };
        frame(0);
        let (before, t) = frame(60);
        stage.paused.store(true, Ordering::Relaxed);
        // Two seconds paused: the same picture, the clock still.
        let (during, t2) = frame(180);
        assert_eq!((0..ROWS).flat_map(|y| (0..160).map(move |x| (x, y))).filter(|&(x, y)| before.get(x, y) != during.get(x, y)).count(), 0);
        assert_eq!(t, t2);
        // Going on: a frame later is a frame on, not two seconds.
        stage.paused.store(false, Ordering::Relaxed);
        let (_, t3) = frame(181);
        assert!((t3 - t - 1.0 / FPS).abs() < 1e-9, "{t} then {t3}");
    }

    #[test]
    fn the_music_follows_the_script_clock() {
        let stage = Stage::new(160);
        stage.seek_to(42.0);
        stage.view_at(0);
        assert!((f64::from_bits(stage.clock.load(Ordering::Relaxed)) - 42.0).abs() < 1e-9);
    }

    #[test]
    fn the_camera_dives_in_the_waves_spread_then_merge_and_it_comes_back_out() {
        let w = 100;
        let placed = script::parse("{dive in}abc{gap}{gap}{dive out}def").unwrap().place(w).unwrap();
        let tl = script::Timeline::new(&placed, w);
        let at = |on| tl.cues.iter().find(|c| c.code == script::Code::Dive(on)).unwrap().t;
        let view = |t| script_view(&tl, 0.0, t).dive;
        let (a, b) = (at(true), at(false));
        // In: the zoom, then the waves.
        assert_eq!(view(a - 0.1), dive::Dive::default());
        assert!(view(a + 0.5).zoom > 0.0 && view(a + 0.5).zoom < 1.0);
        assert_eq!(view(a + 0.5).waves, 0.0);
        assert_eq!(view(a + dive::DIVE_SECS).zoom, 1.0);
        assert!(view(a + dive::DIVE_SECS + 0.3).waves > 0.0);
        assert_eq!(view(a + dive::DIVE_SECS + dive::WAVES_SECS).waves, 1.0);
        // Out: the waves merge first, then the zoom.
        assert_eq!(view(b - 0.1), dive::Dive { zoom: 1.0, waves: 1.0 });
        assert!(view(b + 0.3).waves < 1.0);
        assert_eq!(view(b + 0.3).zoom, 1.0);
        assert_eq!(view(b + dive::WAVES_SECS + 1e-6).waves, 0.0);
        assert!(view(b + dive::WAVES_SECS + 0.5).zoom < 1.0);
        assert_eq!(view(b + dive::WAVES_SECS + dive::DIVE_SECS + 1e-6), dive::Dive::default());
    }

    #[test]
    fn the_pitch_dives_into_the_big_c_after_the_loop() {
        let w = 112;
        let stage = Stage::new(w);
        let dive_in = first_cue(&stage, script::Code::Dive(true));
        let dive_out = first_cue(&stage, script::Code::Dive(false));
        // Once the last rider is out of the loop, while the C is big in it...
        let loopers: Vec<_> = stage.glyphs.iter().filter(|g| g.path == Some(looping::Shape::Loop)).collect();
        let last_out = when_scrolled(&stage, loopers.last().unwrap().x + 4.0 + looping::loop_start(w) + looping::LOOP_EXTRA);
        assert!(dive_in >= last_out, "{dive_in} vs {last_out}");
        stage.seek_to(dive_in);
        assert!(matches!(stage.view_at(0).logo, LogoPhase::Away(t) if t >= wordmark::AWAY_DONE));
        // The water's own text comes on only once the camera is in, the waves out.
        let wet: Vec<_> = stage.glyphs.iter().filter(|g| g.wave == script::Wave::Water && g.path.is_none()).collect();
        assert!(!wet.is_empty());
        let spread = first_cue(&stage, script::Code::Spread);
        assert!(spread >= dive_in + dive::DIVE_SECS);
        assert!(when_scrolled(&stage, wet[0].x) >= spread + dive::WAVES_SECS - 1e-9);
        // What follows comes on only once the camera is all the way out...
        let back_out = dive_out + dive::WAVES_SECS + dive::DIVE_SECS;
        let next = stage.glyphs.iter().find(|g| g.x > wet.last().unwrap().x && g.wave != script::Wave::Water).unwrap();
        assert!(when_scrolled(&stage, next.x) >= back_out - 1e-9, "{} vs {back_out}", when_scrolled(&stage, next.x));
        // ...and before the logo comes home.
        let home = stage.timeline.cues.iter().find(|c| c.code == script::Code::Logo && c.t > dive_in).unwrap().t;
        assert!(back_out <= home, "{back_out} vs {home}");
    }

    #[test]
    fn long_waves_stay_on_screen() {
        // A C64 letter at the top of the long wave, plus the ripple and its
        // outline, under the top border; at the bottom, its extrusion still
        // on screen.
        let up = (scroller::LONG_UP + scroller::SCROLL_WAVE).ceil() as i64;
        let down = (scroller::LONG_DOWN + scroller::SCROLL_WAVE).ceil() as i64;
        let top = SCROLLER_Y + scroller::BASE - 7 - up - 1;
        assert!(top >= 3, "{top}");
        let bottom = SCROLLER_Y + scroller::BASE + down + 2;
        assert!(bottom < (ROWS * 2) as i64, "{bottom}");
    }

    #[test]
    fn logo_clears_the_sky_for_the_long_wave_and_drops_back_in() {
        let w = 112;
        let stage = Stage::new(w);
        let long: Vec<_> = stage.glyphs.iter().filter(|g| matches!(g.wave, script::Wave::Long(_))).collect();
        // Gone, C and all, by the time the first long-wave letter comes on.
        stage.seek_to(when_scrolled(&stage, long[0].x));
        assert_eq!(stage.view_at(0).logo, LogoPhase::Hidden);
        // Still gone while the last one has swelled out and scrolled past the middle.
        let last = long.last().unwrap();
        stage.seek_to(when_scrolled(&stage, last.x + w as f64 / 2.0));
        assert_eq!(stage.view_at(0).logo, LogoPhase::Hidden);
        // Then in again like the first time — not the big C shrinking home.
        let out = first_cue(&stage, script::Code::LogoOut);
        let back = stage.timeline.cues.iter().find(|c| c.code == script::Code::Logo && c.t > out).expect("logo comes back").t;
        stage.seek_to(back + 0.5);
        let LogoPhase::Enter(t) = stage.view_at(0).logo else { panic!("{:?}", stage.view_at(0).logo) };
        assert!((t - 0.5).abs() < 1e-9);
    }

    #[test]
    fn loop_is_as_big_as_fits_under_the_top_border() {
        // Its top (an upside-down letter's outline) stays clear of the
        // border, with little room to spare.
        let top = (SCROLLER_Y + 7) as f64 - 2.0 * looping::LOOP_R - 1.0;
        assert!(top >= layout::TOP_MARGIN as f64, "{top}");
        assert!(top <= layout::TOP_MARGIN as f64 + 8.0, "room for a bigger loop: {top}");
    }

    #[test]
    fn the_big_c_turns_through_the_loop_and_the_dive_then_rests_before_the_waves() {
        let w = 112;
        let stage = Stage::new(w);
        let ride = looping::ride(&stage.glyphs, w).unwrap();
        let cue = |code| first_cue(&stage, code);
        let (dive_in, spread) = (cue(script::Code::Dive(true)), cue(script::Code::Spread));
        let tilt = |t: f64| looping::tilt(stage.timeline.scrolled(t), Some(ride));
        let spinners: Vec<_> = stage.glyphs.iter().filter(|g| g.path == Some(looping::Shape::Spin)).collect();
        assert!(!spinners.is_empty());
        let spin_in = when_scrolled(&stage, spinners[0].x);
        // Turning from the loop on, through the zoom in...
        let first_in = when_scrolled(&stage, ride.0);
        assert!(tilt(first_in + 0.5) > 0.0);
        assert!(dive_in > first_in && dive_in < spin_in);
        // ...the riders on the turning surface come on once the camera is in...
        assert!(spin_in >= dive_in + dive::DIVE_SECS - 1e-9);
        stage.seek_to(spin_in + 1.0);
        let v = stage.view_at(0);
        assert_eq!(v.dive, dive::Dive { zoom: 1.0, waves: 0.0 });
        assert!(tilt(spin_in + 1.0) > 0.0);
        // ...then it slows down, lands level, and only then do the waves spread.
        let rest = when_scrolled(&stage, looping::spin_done(ride));
        assert!(rest > spin_in + 1.0 && spread >= rest - 1.0 / FPS, "{rest} vs {spread}");
        assert_eq!(tilt(spread), 0.0);
        stage.seek_to(spread + dive::WAVES_SECS);
        assert!(stage.view_at(0).dive.waves > 0.999);
        eprintln!(
            "loop+dive {:.2}s · riding the turn {:.2}s (slowing the last {:.2}s) · waves at {:.2}s",
            spin_in - first_in,
            rest - spin_in,
            looping::SPIN_DOWN / FPS,
            spread - first_in
        );
    }

    #[test]
    fn spin_riders_lie_on_the_turned_surface_through_the_middle() {
        let w = 112;
        let cam = dive::Camera::new(w, ROWS * 2, 1.0, 0.3);
        let tilt = 2.0;
        let (sin, cos) = f64::sin_cos(tilt);
        let mid = (w as f64 / 2.0, ROWS as f64);
        for along in [-40.0, -10.0, 0.0, 25.0] {
            let s = cam.rider(along, tilt, 0.3, 8.0);
            let (rx, ry) = (s.x - mid.0, s.y - mid.1);
            // Along the turned line, off it by the slosh only.
            assert!((rx * cos + ry * sin - along).abs() < 1e-9);
            assert!((-rx * sin + ry * cos).abs() < 20.0, "{along}: {}", -rx * sin + ry * cos);
            let d = (s.angle - tilt).rem_euclid(std::f64::consts::TAU);
            assert!(d.min(std::f64::consts::TAU - d) < 1.2, "angle {} vs tilt {tilt}", s.angle);
        }
    }

    #[test]
    fn logo_steps_aside_for_the_loop_and_comes_back() {
        let w = 112;
        let stage = Stage::new(w);
        let away = first_cue(&stage, script::Code::LogoAway);
        let back = stage.timeline.cues.iter().find(|c| c.code == script::Code::Logo && c.t > away).expect("logo comes back").t;
        let ride = looping::ride(&stage.glyphs, w).unwrap();
        // Out of the way before the first rider comes in...
        let first_in = when_scrolled(&stage, ride.0);
        assert!(away + wordmark::AWAY_DONE <= first_in, "{away} + away vs {first_in}");
        // ...and back only once the last is off the C.
        let last_out = when_scrolled(&stage, ride.1);
        assert!(back >= last_out, "{back} vs {last_out}");
        stage.seek_to(away + 0.3);
        let LogoPhase::Away(t) = stage.view_at(0).logo else { panic!("logo not stepping aside") };
        assert!((t - 0.3).abs() < 1e-9);
        stage.seek_to(back - 0.1);
        assert!(matches!(stage.view_at(0).logo, LogoPhase::Away(_)), "the big C stays for the whole ride");
        stage.seek_to(back + 0.5);
        let LogoPhase::Return(t) = stage.view_at(0).logo else { panic!("logo not back") };
        assert!((t - 0.5).abs() < 1e-9);
    }

    #[test]
    fn logo_waits_for_its_code() {
        let stage = Stage::new(120);
        let cue = first_cue(&stage, script::Code::Logo);
        stage.seek_to(cue - 0.5);
        assert_eq!(stage.view_at(0).logo, LogoPhase::Hidden);
        stage.seek_to(cue + 1.0);
        let LogoPhase::Enter(t) = stage.view_at(0).logo else { panic!("logo not entering") };
        assert!((t - 1.0).abs() < 1e-9);
        // Before the code (scroller running): no landscape, no logo.
        stage.seek_to(stage.intro.secs + 0.5);
        let v = stage.view_at(0);
        assert_eq!((v.logo, v.heading, v.panel, v.hills), (LogoPhase::Hidden, None, None, None));
        assert!(v.scroll > 0.0);
    }

    #[test]
    fn screen_is_empty_of_text_when_the_logo_drops() {
        for w in [80, 112, 200] {
            let stage = Stage::new(w);
            let cue = first_cue(&stage, script::Code::Logo);
            let scrolled = stage.timeline.scrolled(cue);
            for g in &stage.glyphs {
                let x = w as f64 - scrolled + g.x;
                let wide = scroller::advance(g.font, g.ch) as f64;
                assert!(x <= -wide || x >= w as f64, "{w}: {:?} on screen at x={x}", g.ch);
            }
        }
    }

    #[test]
    fn cold_open_plays_before_the_scroller() {
        let stage = Stage::new(112);
        for t in [0.1, 5.0, stage.intro.secs - 0.1] {
            stage.seek_to(t);
            let v = stage.view_at(0);
            assert_eq!((v.intro, v.hills, v.logo), (Some(t), None, LogoPhase::Hidden));
            // No scroll text on screen yet.
            assert!(stage.glyphs.iter().all(|g| 112.0 - v.scroll + g.x >= 112.0), "t={t}");
        }
        // The DYCP may still be crossing as the text starts; once it's
        // gone, so is the cold open.
        stage.seek_to(stage.intro.shown + 0.1);
        assert_eq!(stage.view_at(0).intro, None);
    }

    #[test]
    fn hills_build_while_the_opening_line_scrolls_by() {
        for w in [80, 112, 200] {
            let stage = Stage::new(w);
            let hills = stage.timeline.since(script::Code::Hills, f64::MAX).unwrap();
            let logo = first_cue(&stage, script::Code::Logo);
            // The opening line comes on screen soon after the landscape
            // starts (the scroll text may hold it back a moment)...
            let soon = (0..=5 * FPS as usize).map(|f| hills + f as f64 / FPS).find(|&t| {
                let scrolled = stage.timeline.scrolled(t);
                stage.glyphs.iter().any(|g| (0.0..w as f64).contains(&(w as f64 - scrolled + g.x)))
            });
            assert!(soon.is_some(), "{w}: no text on screen within 5 s of the hills");
            // ...and the landscape has (nearly) finished building by the time the logo drops.
            assert!(logo - hills >= landscape::ENTRANCE_SECS - 1.0, "{w}: only {:.1}s to build", logo - hills);
        }
    }

    #[test]
    fn scroller_steps_exactly_one_pixel_a_frame() {
        // From the start, and after jumping to each code: the text moves
        // one whole pixel every frame (0 while paused) — never 0-then-2.
        let stage = Stage::new(112);
        let mut starts = vec![0.0];
        starts.extend(stage.timeline.cues.iter().map(|c| c.t));
        for start in starts {
            stage.seek_to(start);
            let px: Vec<i64> = (0..600).map(|f| scroller::pen(stage.view_at(f).scroll)).collect();
            let mut wraps = 0;
            for (f, w) in px.windows(2).enumerate() {
                let step = w[1] - w[0];
                // The one allowed jump: the show looping back to the start.
                if step < 0 {
                    wraps += 1;
                    continue;
                }
                assert!(step == 0 || step == 1, "from {start:.3}s, frame {f}: step {step}");
            }
            assert!(wraps <= 1);
            // Not stuck either: it does move.
            assert!(px[599] != px[0]);
        }
    }

    #[test]
    fn bye_clears_the_stage_before_the_show_loops() {
        let stage = Stage::new(112);
        let bye = stage.timeline.since(script::Code::Bye, f64::MAX).expect("pitch has a {bye}");
        stage.seek_to(bye - 0.1);
        let v = stage.view_at(0);
        assert!(matches!(v.logo, LogoPhase::Enter(_) | LogoPhase::Return(_)) && v.hills.is_some());
        // Leaving: the logo jumps away, the landscape plays its entrance backwards.
        stage.seek_to(bye + 0.5);
        let v = stage.view_at(0);
        assert_eq!(v.logo, LogoPhase::Exit(0.5));
        assert!(v.hills.unwrap() < landscape::ENTRANCE_SECS);
        // All gone before the text wraps and the show starts over.
        assert!(stage.timeline.total - bye >= wordmark::EXIT_DONE.max(landscape::EXIT_SECS));
        stage.seek_to(stage.timeline.total - 0.05);
        let v = stage.view_at(0);
        assert_eq!((v.logo, v.hills), (LogoPhase::Hidden, None));
    }

    #[test]
    fn space_jumps_to_the_next_code() {
        let stage = Stage::new(120);
        stage.seek_to(0.0);
        let next = |frame| {
            *stage.seek.lock().unwrap() = Some(Seek::Next);
            stage.view_at(frame)
        };
        // The cold open's steps 2, 3 and 4, then the scroller starting.
        assert_eq!(next(0).intro, Some(stage.intro.marks[0]));
        assert_eq!(next(0).intro, Some(stage.intro.marks[1]));
        assert_eq!(next(0).intro, Some(stage.intro.marks[2]));
        let v = next(0);
        assert_eq!(v.hills, None);
        // Then the hills come in (and the 808 is launched).
        let v = next(0);
        assert_eq!((v.hills, v.logo), (Some(0.0), LogoPhase::Hidden));
        // The bass's code, with the coloured text.
        assert_eq!(next(0).logo, LogoPhase::Hidden);
        // Then the logo drops — and the jump sticks: a second later it's a second in.
        assert_eq!(next(0).logo, LogoPhase::Enter(0.0));
        assert_eq!(stage.view_at(60).logo, LogoPhase::Enter(1.0));
    }

    #[test]
    fn show_starts_over_after_the_text() {
        let stage = Stage::new(120);
        stage.seek_to(stage.timeline.total + 0.5);
        let v = stage.view_at(0);
        assert_eq!((v.intro, v.hills, v.logo), (Some(0.5), None, LogoPhase::Hidden));
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
