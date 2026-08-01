//! VectorField — animated 2D vector field rendered as Unicode arrows.
//!
//! Each cell shows the local direction of a time-evolving flow as one of
//! eight arrows (← ↖ ↑ ↗ → ↘ ↓ ↙) and is coloured by proximity to one of
//! 3 vortex cores that orbit slowly across the buffer.
//!
//! Run: `cargo run --release --example vector_field`

use chromakopia::prelude::*;

// Octant order in every set: E, SE, S, SW, W, NW, N, NE.

/// Default — thin unicode arrows.
const ARROWS: [char; 8] = ['→', '↘', '↓', '↙', '←', '↖', '↑', '↗'];

/// Flow lines without arrowheads — like wind streaks; calmer, more abstract.
const FLOW: [char; 8] = ['─', '╲', '│', '╱', '─', '╲', '│', '╱'];

/// ASCII-only fallback for terminals without good unicode coverage.
const ASCII: [char; 8] = ['>', '\\', 'v', '/', '<', '\\', '^', '/'];

#[tokio::main]
async fn main() {
    // Multi-stop gradient: calm zones → vortex cores.
    // white → light grey → darker grey → dark red → red → bright red.
    // Toggle to monochrome by swapping which line is commented.
    let palette = gradient(&["#ffffff", "#ff3333"]).palette(64);
    // let palette = vec![Color::new(200, 200, 200)];

    // Three sets cycle every 5 seconds, with a ~1.7s stochastic dissolve
    // at the end of each cycle — cells flip independently into the next set.
    let cycle = vec![ARROWS, FLOW, ASCII];

    let field = VectorField::new()
        .palette(palette)
        .arrows(ARROWS)
        .arrow_cycle(cycle, 5.0)
        .speed(1.0)        // 1.0 = default. Bigger = faster swirls.
        .scale(1.0)        // 1.0 = default. Bigger = smaller, busier eddies.
        .spacing(2, 1);    // every 2 cols × every row. (1,1) = full density.

    // 3 presets × 5s each = 15s per loop. Run for ~3 loops.
    Scene::new()
        .add(field)
        .quit_key('q')
        .run(45.0)
        .await;
}
