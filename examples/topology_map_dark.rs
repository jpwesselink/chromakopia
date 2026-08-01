//! Topology map — dark variant. Transparent background; the terminal's
//! default bg shows through, and contours render in pale pink → mid red
//! through the dynamic level normalisation. Reads as soft glowing isolines
//! floating in dark space.
//!
//! Run: `cargo run --release --example topology_map_dark`

use chromakopia::prelude::*;

#[tokio::main]
async fn main() {
    // Transparent background — terminal default shows through. Lines
    // stay light overall: pale pink for lowest, mid red for highest.
    let palette = gradient(&[
        "#f0a8a8",  // pale pink — lowest lines
        "#d06868",
        "#a03030",  // mid red — highest lines
    ]).palette(24);

    let map = HeightMap::new()
        .palette(palette)
        .speed(1.0)            // 1.0 = default. Drift rate of the peaks.
        .interval(0.2)         // vertical distance between contour rings; smaller = denser.
        .thickness(0.02)       // line "thickness" in field units.
        .peaks(8)              // 6 = default; more peaks = busier topology.
        .sigma(22.0)           // larger σ = gentler hills, smoother lines.
        .size(80, 96);         // 4× the default vertical extent.

    Scene::new()
        .add(map)
        .quit_key('q')
        .run(30.0)
        .await;
}
