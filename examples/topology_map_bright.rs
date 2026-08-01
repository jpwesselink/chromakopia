//! Topology map — bright variant. Beige paper background with solid black
//! contour lines. No per-line colour variation; reads like a classic
//! hand-drawn topographic map.
//!
//! Run: `cargo run --release --example topology_map_bright`

use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    // Single black for every contour line; beige paper background.
    let palette = vec![Color::new(40, 30, 20)]; // near-black with a hint of warmth
    let paper = Color::new(240, 232, 208);

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
