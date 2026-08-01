//! Greetings — a cracktro-style scroller with cued greetings.
//!
//! A long message scrolls across mid-screen; when it reaches its `{cue1}`
//! marker, name marquees pop in above and below and bounce. Cue timing is
//! computed from the marker's position in the text — move the marker,
//! move the moment. The message is plain text, so it can be arbitrarily
//! long and edited freely.
//!
//! Run: `cargo run --release --example greetings`   (press q to quit)

use chromakopia::prelude::*;

const MESSAGE: &str = "Hey, thanks for watching. I'd like to thank some people for \
congratulating me for my birthday.{cue1} And now that everyone is here, let's talk \
chromakopia: terminal animations, framebuffer scenes, marquees riding sine waves, \
topology maps, vector fields, wind, glitch, plasma... all of it plain characters \
in your terminal, no pixels harmed. Thanks for scrolling by!";

/// Cells per second for the message row — reading speed caps this one.
const SCROLL_SPEED: f64 = 16.0;

/// Strip `{cueN}` markers from `msg`; return the display text and each
/// cue's `(n, char_index_in_display_text)`.
fn parse_cues(msg: &str) -> (String, Vec<(u32, usize)>) {
    let mut display = String::with_capacity(msg.len());
    let mut cues = Vec::new();
    let mut rest = msg;
    while let Some(start) = rest.find("{cue") {
        display.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let Some(end) = after.find('}') else {
            display.push_str(&rest[start..]);
            return (display, cues);
        };
        if let Ok(n) = after[..end].parse::<u32>() {
            cues.push((n, display.chars().count()));
        }
        rest = &after[end + 1..];
    }
    display.push_str(rest);
    (display, cues)
}

/// Seconds until the char at `idx` reaches the center of the screen.
fn cue_secs(idx: usize, term_w: usize) -> f64 {
    (idx as f64 - term_w as f64 / 2.0).max(0.0) / SCROLL_SPEED
}

/// A greeting row: blank until `at` seconds, then a bouncing marquee.
fn name_row(name_block: impl Effect, at: f64, speed: f64, dir: ScrollDirection) -> impl Effect {
    DelayedStart::new(
        at,
        VSineWave::wrap(Marquee::wrap(name_block).speed(speed).direction(dir))
            .amplitude(1.0)
            .frequency(0.3)
            .speed(0.15),
    )
}

#[tokio::main]
async fn main() {
    let (display, cues) = parse_cues(MESSAGE);
    let cue1_idx = cues
        .iter()
        .find(|(n, _)| *n == 1)
        .map(|&(_, i)| i)
        .expect("MESSAGE must contain a {cue1} marker");
    let term_w = terminal_width();
    let cue1 = cue_secs(cue1_idx, term_w);

    // Breather after the message before its loop repeats.
    let message = format!("{display}{}", " ".repeat(40));
    // Long enough to read the whole message once, plus a tail.
    let total = display.chars().count() as f64 / SCROLL_SPEED + 4.0;

    let label = Color::new(110, 110, 110);

    Scene::new()
        .add(text("greetings — watch for the birthday cue", label))
        .add(name_row(
            Glow::on("\nsuper_jo_nathan          \n"),
            cue1,
            10.0,
            ScrollDirection::Left,
        ))
        .add(Marquee::wrap(Rainbow::on(&message)).speed(SCROLL_SPEED))
        .add(name_row(
            Neon::on("\nRobbert-Jan          \n"),
            cue1 + 0.4,
            12.0,
            ScrollDirection::Right,
        ))
        .add(name_row(
            Rainbow::on("\njanglad          \n"),
            cue1 + 0.8,
            8.0,
            ScrollDirection::Left,
        ))
        .quit_key('q')
        .run(total)
        .await;
}
