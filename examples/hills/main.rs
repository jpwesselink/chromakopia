//! Hills — stacked dithered ridge layers swaying left and right.
//!
//! Every layer is a row of rounded humps (|sin| — round tops, sharp
//! valleys) filled with a 2×2 ordered-dither mix of two pastels: sky,
//! clouds, mint hills and peach ground. All layers make the same eased
//! back-and-forth swing, each a little out of phase with the one behind
//! it, so the motion ripples through the stack front to back. A band of
//! cloud shadows drifts across the land as a `Multiply` layer.
//!
//! Last, centred over it all: CHROMAKOPIA in chunky Mario-title-style
//! letters — pastel pink fading to lavender with a cream top edge, deep
//! plum outline and 3D extrusion down-right. The letters drop in one by
//! one from above the screen with a bounce, then each grows into its own
//! DYCP sine. They're painted left to right, each fully before the next,
//! so every letter covers its left neighbour's extrusion.
//!
//! Along the bottom, the MIT licence scrolls by in the C64 character ROM
//! font, same pastel styling, riding a gentle sine — cracktro style.
//!
//! Run: `cargo run --release --example hills`   (press q to quit)

mod figlet;
mod glyphs;
mod landscape;
mod scroller;
mod wordmark;

use chromakopia::prelude::*;

/// Scene height in terminal rows (two pixels each).
pub const ROWS: usize = 40;
/// Rows of open sky above the landscape, so the wordmark has headroom.
pub const TOP_PAD: usize = 6;

fn build_layers(screen_w: usize) -> Layers {
    let (word, x0, y0) = wordmark::Wordmark::placed(screen_w);
    // Baseline high enough that the scroller's wave plus extrusion stays on screen.
    let scroller = scroller::Scroller::new(scroller::LICENSE_TEXT, (ROWS * 2) as i64 - 15);
    landscape::landscape()
        .layer(Layer::paint(move |c| word.paint(c, x0, y0)))
        .layer(Layer::paint(move |c| scroller.paint(c)))
}

#[tokio::main]
async fn main() {
    Scene::new()
        .add(build_layers(terminal_width()))
        .fullscreen()
        .quit_key('q')
        .run(3600.0)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_render_without_panicking() {
        let layers = build_layers(120);
        let mut b = FrameBuffer::new(120, ROWS);
        layers.render(&mut b, 260);
        assert!(b.to_ansi_string().contains('▀'));
    }
}
