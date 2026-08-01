//! Parallax demo — three `Marquee` layers occupying the SAME vertical
//! region, overlaid on top of each other. Stars fill the whole sky; the
//! mountain silhouette sits over the bottom half; trees stand over the
//! mountains at ground level. Each scrolls at a different speed; the
//! closer layer occludes the layer behind because Marquee skips spaces.
//!
//! Run: `cargo run --release --example parallax`

use chromakopia::animate::framebuffer::Cell as RawCell;
use chromakopia::prelude::*;

/// Forest-floor gradient. Fills its band with full-block cells whose
/// colour lerps `far` (top, deep in the woods) → `near` (bottom, closest
/// to camera). Static — doesn't scroll or swing.
struct Ground {
    near: Color,
    far: Color,
    height: usize,
}
impl Effect for Ground {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        let h = self.height.min(buf.height);
        for y in 0..h {
            let t = if h > 1 {
                y as f64 / (h - 1) as f64
            } else {
                0.0
            };
            let color = Color::lerp_rgb(self.far, self.near, t);
            // bg = fg so overlays composited on top inherit this colour.
            for x in 0..buf.width {
                buf.set(x, y, RawCell::with_bg('█', color, color));
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (80, self.height)
    }
}

/// Mountain silhouette lit from the right (where the implied moon hangs).
/// Cells on
/// the right side of each peak/range render with `lit`; left-side cells
/// render with `shadow`. A simple horizontal lerp keyed off the cell's
/// distance into its mountain block — preserves the per-peak shading even
/// as the marquee scrolls.
struct LitMountains {
    art: Vec<Vec<char>>,
    shadow: Color,
    lit: Color,
}
impl LitMountains {
    fn new(text: &str, shadow: Color, lit: Color) -> Self {
        let art: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        Self { art, shadow, lit }
    }
}
impl Effect for LitMountains {
    fn render(&self, buf: &mut FrameBuffer, _frame: usize) {
        for (y, line) in self.art.iter().enumerate() {
            if y >= buf.height {
                break;
            }
            // For this row, identify contiguous runs of non-space cells.
            // Within each run, lerp shadow→lit from left to right.
            let mut x = 0usize;
            while x < line.len() && x < buf.width {
                if line[x] == ' ' {
                    x += 1;
                    continue;
                }
                // Find the end of this run.
                let mut end = x;
                while end < line.len() && end < buf.width && line[end] != ' ' {
                    end += 1;
                }
                let run_len = (end - x).max(1) as f64;
                for cx in x..end {
                    let t = (cx - x) as f64 / run_len;
                    let color = Color::lerp_rgb(self.shadow, self.lit, t);
                    // Mark this mountain cell as a solid bg layer so
                    // anything overlaid on top inherits this colour.
                    buf.set(cx, y, RawCell::with_bg(line[cx], color, color));
                }
                x = end;
            }
        }
    }
    fn size(&self) -> (usize, usize) {
        (
            self.art.iter().map(|l| l.len()).max().unwrap_or(0),
            self.art.len(),
        )
    }
}

// 14-row starfield — fills the full scene height.
const STARS: &str = "
   *     .        *           .      *        .            *        .       *
        .             *        .         *          .                  *
  *           .         *              .            *              .            *
       *            .        *               .          *       .                 *
 .          *           .         *      .                  *           .
     *           .            *      .          *         .                       *
        *              .              *            .              *           .
   .          *               .            *             .         *
      *            .       *           .              *           .         *
 .           *            .            *      .                  *           .
       *            .        *               .          *       .                 *
  *           .         *              .            *              .            *
        .             *        .         *          .                  *
   *     .        *           .      *        .            *        .       *";

// Far mountains — proper triangular silhouettes that taper smoothly from
// peak to base. 9 rows tall. Adjacent mountain skirts overlap at the
// bottom so the ridge line reads as a continuous range, not "peaks
// stamped onto a wall."
const FAR_MOUNTAINS: &str = "
       █                      █                         █                       █
      ███                    ███                       ███                     ███
     █████        █         █████          █         █████        █          █████
    ███████      ███       ███████        ███       ███████      ███        ███████
   █████████    █████     █████████      █████     █████████    █████      █████████
  ███████████  ███████   ███████████    ███████   ███████████  ███████    ███████████
 █████████████████████████████████████████████████████████████████████████████████████
███████████████████████████████████████████████████████████████████████████████████████
███████████████████████████████████████████████████████████████████████████████████████";

// 7-row FILLED mountain silhouette. Every mountain cell is a solid block
// so nothing shines through. The shape is implicit in which cells are
// filled vs space (sky).
const MOUNTAINS: &str = "
                █                                                        █
               ███                █                                     ███
              █████              ███           █                       █████
         ██  ████████        █  █████         ███   █       █         ███████
        ████████████████    ███████████     ███████ ███   █████     ███████████. 
   ███████████████████████ ██████████████ ███████████████████████████████████████
███████████████████████████████████████████████████████████████████████████████████";

// 6-row foreground tree line with VARIED heights — not every tree is the
// same size. Same ground line, but the crown tops sit at different rows:
//   • tall   (col 10, 60): crown rows 0-3 (4 rows)
//   • medium (col 40): rows 1-3
//   • short  (col 22, 72): rows 2-3
//   • tiny   (col 30, 50): row 3 only
// Trunks aligned under each crown.
// Front: 6 rows with varied heights — tall ▲ to tiny ▲. Trunks aligned
// under each crown via the python-generated positions.
const TREES: &str = "
          ▲                                                 ▲
         ▲█▲                            ▲                  ▲▲▲
        ▲███▲         ▲                ▲▲▲                ▲▲▲▲▲         ▲
       ▲█████▲       ▲▲▲      ▲       ▲▲▲▲▲       ▲      ▲▲▲▲▲▲▲       ▲▲▲
          █           █       |         █         |         █           █
         ▐█▌          █       |        ▐█▌        |        ▐█▌          █";

// Distant trees — tiny ▲ silhouettes along the horizon. No trunks.
const DISTANT_TREES: &str = "

        ▲            ▲           ▲            ▲           ▲           ▲
   ▲   ▲▲▲    ▲     ▲▲▲     ▲   ▲▲▲     ▲    ▲▲▲    ▲    ▲▲▲   ▲     ▲▲▲   ▲";

// Far-mid: 4 rows. Small ▲ crowns with single-pipe trunks.
const FAR_MID_TREES: &str = "
       ▲                ▲                 ▲                 ▲              ▲
      ▲▲▲              ▲▲▲               ▲▲▲               ▲▲▲            ▲▲▲
     ▲▲▲▲▲     ▲      ▲▲▲▲▲     ▲       ▲▲▲▲▲       ▲     ▲▲▲▲▲     ▲    ▲▲▲▲▲
       |       |        |       |         |         |       |       |      |";

// Mid: 6 rows, varied ▲ heights, full trunks.
const MID_TREES: &str = "
     ▲                             ▲                               ▲
    ▲▲▲          ▲                ▲▲▲                  ▲          ▲▲▲
   ▲▲▲▲▲        ▲▲▲              ▲▲▲▲▲                ▲▲▲        ▲▲▲▲▲        ▲
  ▲▲▲▲▲▲▲      ▲▲▲▲▲       ▲    ▲▲▲▲▲▲▲        ▲     ▲▲▲▲▲      ▲▲▲▲▲▲▲      ▲▲▲
     █           █         |       █           |       █           █          █
    ▐█▌         ▐█▌        |      ▐█▌          |      ▐█▌         ▐█▌         █";

#[tokio::main]
async fn main() {
    // All layers share the same cycle + easing curve — they accelerate
    // and decelerate IN SYNC, like a camera panning. The only difference
    // is the per-layer speed multiplier: foreground moves further per
    // cycle, background barely creeps. Classic parallax depth cue.
    // Long cycle = slow, lazy swing in each direction.
    let cycle_secs = 12.0;
    let easing = Easing::EaseInOut;
    let base_speed = 3.0;

    // Realistic star colours — mostly white with subtle warm / cool tints.
    let star_palette = gradient(&[
        "#9ab5ff", // cool blue (hot star)
        "#ffffff", // pure white
        "#fff3c8", // warm white
        "#ffd28a", // amber (cool star)
    ])
    .palette(8);
    let stars = Marquee::wrap(Plasma::on(STARS).palette(star_palette).seed(7.0))
        .speed(base_speed * 1.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 14);

    // Far mountains: moonlit. Shadow side (left of each ridge) very dark,
    // lit side (right, toward moon) a touch brighter.
    let far_mountains = Marquee::wrap(LitMountains::new(
        FAR_MOUNTAINS,
        Color::new(28, 32, 48),
        Color::new(50, 58, 78),
    ))
    .speed(base_speed * 2.0)
    .direction(ScrollDirection::Left)
    .cycle(cycle_secs, easing)
    .swing()
    .size(80, 9);

    // Near mountains: moonlit, brighter range overall (closer to camera).
    let mountains = Marquee::wrap(LitMountains::new(
        MOUNTAINS,
        Color::new(48, 52, 76),
        Color::new(95, 102, 130),
    ))
    .speed(base_speed * 4.0)
    .direction(ScrollDirection::Left)
    .cycle(cycle_secs, easing)
    .swing()
    .size(80, 7);

    // (Moon glyph removed — implied light source. Mountain shading kept.)

    // Forest floor. Spans the 5 lowest rows of the scene (9-13), brown
    // gradient that darkens with depth: lightest at the bottom (closest
    // to camera), darkest at the top (deep in the woods).
    // Brown ground covers only the bottom 3 rows — the front-most pair of
    // tree layers stand on it, while the two distant tree layers still
    // sit on the near-mountain slopes (their bottoms at row 9 and 10).
    let ground = Ground {
        near: Color::new(110, 75, 40), // warm medium brown
        far: Color::new(45, 30, 18),   // dark earth where it meets the mountains
        height: 3,
    };

    // Each tree layer uses a single solid green — no plasma, no shimmer.
    // Atmospheric perspective: back layers darker, front layers brighter.
    let distant_trees = Marquee::wrap(Solid(Color::new(21, 51, 30)).on(DISTANT_TREES))
        .speed(base_speed * 5.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 3);

    let far_mid_trees = Marquee::wrap(Solid(Color::new(26, 69, 39)).on(FAR_MID_TREES))
        .speed(base_speed * 6.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 4);

    let mid_trees = Marquee::wrap(Solid(Color::new(31, 80, 38)).on(MID_TREES))
        .speed(base_speed * 7.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 6);

    let trees = Marquee::wrap(Solid(Color::new(47, 122, 61)).on(TREES))
        .speed(base_speed * 10.0)
        .direction(ScrollDirection::Left)
        .cycle(cycle_secs, easing)
        .swing()
        .size(80, 6);

    // Layer back-to-front: stars, far mountains, near mountains, mid
    // trees, front trees. Mid trees sit at rows 10-13 (4 rows, offset -4)
    // — same ground line as front trees but smaller crowns, so they peek
    // through gaps between front trees.
    // Tree vertical-offset stack: every layer floats one row above the
    // very bottom so even the front trees have a strip of ground (the
    // mountain base) below them. Back layers float progressively higher,
    // matching distance-to-horizon perspective.
    Scene::new()
        .add(stars)
        .overlay(far_mountains, 9, -11)
        .overlay(mountains, 7, -9)
        .overlay(ground, 3, -3) // brown floor, rows 11-13 (mountain bases ↓)
        .overlay(distant_trees, 3, -7) // bottom at row 9
        .overlay(far_mid_trees, 4, -7) // bottom at row 10
        .overlay(mid_trees, 6, -8) // bottom at row 11
        .overlay(trees, 6, -7) // bottom at row 12
        .quit_key('q')
        .run(40.0)
        .await;
}
