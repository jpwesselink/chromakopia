//! Topology map — bright with gradient. Beige paper background. Lowest
//! contours fade *into* the paper, peaks burn deep burgundy. Looks like an
//! old aged topographic map where worn ink at low elevations has faded.
//!
//! Run: `cargo run --release --example topology_map_bright_gradient`

use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    let paper = Color::new(240, 232, 208); // cream paper

    // First stop matches the paper colour so the lowest contours fade
    // into the page. Peaks burn near-black burgundy.
    let palette = gradient(&[
        "#f0e8d0",  // paper — lowest lines disappear
        "#d0a890",
        "#a04040",
        "#1a0000",  // near-black burgundy — highest lines
    ])
    .palette(32);

    let map = HeightMap::new()
        .palette(palette)
        .bg(paper)
        .speed(1.0)
        .interval(0.4)
        .thickness(0.02)
        .peaks(8)
        .sigma(16.0);

    Scene::new()
        .add(map)
        .quit_key('q')
        .run(30.0)
        .await;
}
