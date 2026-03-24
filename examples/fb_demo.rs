use chromakopia::animate::*;
use chromakopia::{center, presets};
use std::time::Duration;

const BANNER: &str = r#"   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|"#;

const LICENSE: &str = "\
MIT License

Copyright (c) 2026 JP Wesselink

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.";

#[tokio::main]
async fn main() {
    let bg = chromakopia::bg_color();
    let storm = presets::storm().palette(256);
    let mist = presets::mist().palette(256);
    let fps = 60;
    let alinea_delay = fps / 2; // 0.5s between alineas

    // Split license into alineas (paragraphs separated by blank lines)
    let alineas: Vec<&str> = LICENSE.split("\n\n").collect();

    // Build full text block: banner + blank + license
    let full = format!("{}\n\n{}", BANNER, LICENSE);
    let centered = center(&full);
    let centered_lines: Vec<&str> = centered.lines().collect();

    let banner_height = BANNER.lines().count();

    let mut scene = Scene::new();

    // Banner — elastic scroll + plasma composite
    let banner_centered: Vec<&str> = centered_lines[..banner_height].to_vec();
    for l in &banner_centered {
        scene = scene.line(Line::full(l, Chain::new()
            .then(fps * 3, Fade::in_from(
                Composite::new(
                    Scroll::new(l, storm.clone(), ScrollDirection::Left, Easing::Elastic(0.15), fps * 3, 1),
                    Plasma::new(l, storm.clone(), 42.0),
                ),
                bg, Easing::EaseOut, fps,
            ))
            .then(fps * 100, Plasma::new(l, storm.clone(), 42.0))
        ));
    }

    scene = scene.line(Line::blank());

    // License — each alinea alternates left/right, delayed by 0.5s per alinea
    let license_start_line = banner_height + 1; // after banner + blank
    let mut current_line = license_start_line;
    for (alinea_idx, alinea) in alineas.iter().enumerate() {
        let direction = if alinea_idx % 2 == 0 { ScrollDirection::Left } else { ScrollDirection::Right };
        let delay_frames = alinea_idx * alinea_delay;
        let alinea_lines: Vec<&str> = alinea.lines().collect();

        for _ in &alinea_lines {
            let l = centered_lines[current_line];
            // Scroll starts at delay_frames. Before that: invisible (blank).
            // After scroll settles: glow.
            scene = scene.line(Line::full(l, DelayedStart::new(
                delay_frames,
                Chain::new()
                    .then(fps * 2, Fade::in_from(
                        Scroll::new(l, mist.clone(), direction, Easing::Elastic(0.25), fps * 2, 0),
                        bg, Easing::EaseOut, fps,
                    ))
                    .then(fps * 100, Glow::new(l, mist.clone()))
            )));
            current_line += 1;
        }

        // Blank line between alineas (if not the last)
        if alinea_idx < alineas.len() - 1 {
            scene = scene.line(Line::blank());
            current_line += 1;
        }
    }

    // Total duration: wait for last alinea to finish
    let last_delay = alineas.len() * alinea_delay;
    let total_secs = ((last_delay + fps * 3) as f64 / fps as f64) + 5.0;

    scene.run(Duration::from_secs(total_secs as u64)).await;
}
