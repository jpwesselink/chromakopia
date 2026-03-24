use chromakopia::{animate, presets};
use chromakopia::animate::{Easing, FadeKind, FadeTarget, ScrollDirection, TimeRange};

const BANNER: &str = r#"
   ________  ______  ____  __  ______    __ ______  ____  _______
  / ____/ / / / __ \/ __ \/  |/  /   |  / //_/ __ \/ __ \/  _/   |
 / /   / /_/ / /_/ / / / / /|_/ / /| | / ,< / / / / /_/ // // /| |
/ /___/ __  / _, _/ /_/ / /  / / ___ |/ /| / /_/ / ____// // ___ |
\____/_/ /_/_/ |_|\____/_/  /_/_/  |_/_/ |_\____/_/   /___/_/  |_|
"#;

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
    let banner = BANNER.trim_matches('\n');
    let full_text = chromakopia::pad(&format!("{}\n\n{}", banner, LICENSE));
    let banner_lines = banner.lines().count();
    let total_lines = full_text.lines().count();

    let fps = 30;
    let frames_per_line = 90;
    let stagger = 1;
    let scroll_secs = ((total_lines - 1) * stagger + frames_per_line) as f64 / fps as f64;
    let total = scroll_secs + 10.0;

    // Banner slides in as one block from the right
    let banner_scroll = animate::scroll_eased_gradient_effect(
        ScrollDirection::Right, Easing::Elastic(0.25), presets::storm(),
        frames_per_line,
    );
    let license_scroll = animate::scroll_staggered_effect(
        ScrollDirection::Left, Easing::Elastic(0.25), presets::storm(),
        frames_per_line, stagger,
    );

    let seed: f64 = rand::random::<f64>() * 1000.0;
    let bg = chromakopia::bg_color().to_string();
    let palette = chromakopia::gradient(&[&bg, "#cc0000", "#ff2200", "#ff6600", "#ffaa00", "#ffdd00", "#ffffff", "#ffdd00", "#ffaa00", "#ff2200", &bg]);
    // Same palette for both, banner shifted 50% in the color cycle
    let plasma_banner = animate::plasma_seeded_effect(
        palette.clone(), 0.0, seed,
    );
    let plasma_license = animate::plasma_seeded_effect(
        palette, banner_lines as f64 + 1.0, seed,
    );
    let split = banner_lines + 1;
    // Frame offset for 50% palette shift on the banner
    let palette_offset = 40; // ~half a plasma color cycle

    let position_fn = move |text: &str, frame: usize| -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let banner_text = lines[..banner_lines].join("\n");
        let rest_text = lines[split..].join("\n");
        let banner_out = banner_scroll(&banner_text, frame);
        let rest_out = license_scroll(&rest_text, frame);
        format!("{}\n\n{}", banner_out, rest_out)
    };

    let color_fn = move |text: &str, frame: usize| -> String {
        let lines: Vec<&str> = text.split('\n').collect();
        let banner_text = lines[..banner_lines].join("\n");
        let rest_text = lines[split..].join("\n");
        let banner_out = plasma_banner(&banner_text, frame + palette_offset);
        let rest_out = plasma_license(&rest_text, frame);
        format!("{}\n\n{}", banner_out, rest_out)
    };

    let combined = animate::composite(position_fn, color_fn);

    animate::Sequence::new(&full_text)
        .effect(TimeRange::new(0.0, total), fps as u64, combined)
        .fade(
            TimeRange::new(total - 2.0, total),
            FadeKind::FadeTo(FadeTarget::Foreground),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}
