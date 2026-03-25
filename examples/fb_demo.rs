use chromakopia::animate::*;
use chromakopia::{pad, presets};
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
    let fg = chromakopia::fg_color();
    let bg_hex = bg.to_string();
    let storm = presets::storm().palette(256);
    let fire = chromakopia::gradient(&["#ffffff", &bg_hex, "#ff69b4", "#00cccc", "#fffacd", "#8b4513", &bg_hex, "#ffffff"]).palette(256);
    let fps = 30;
    let total = fps * 15;

    // Split license: everything except last alinea, and the last alinea
    let alineas: Vec<&str> = LICENSE.split("\n\n").collect();
    let license_head = alineas[..alineas.len() - 1].join("\n\n");
    let license_tail = alineas[alineas.len() - 1];

    let full = pad(&format!("{}\n\n{}\n\n{}", BANNER, license_head, license_tail));
    let lines: Vec<&str> = full.lines().collect();

    let banner_height = BANNER.lines().count();
    let banner_text: String = lines[..banner_height].join("\n");

    let head_start = banner_height + 1;
    let tail_height = license_tail.lines().count();
    let head_end = lines.len() - tail_height;
    let license_head_text: String = lines[head_start..head_end - 1].join("\n");
    let license_tail_text: String = lines[head_end..].join("\n");

    // Y offsets for continuous plasma field
    let license_head_height = license_head_text.lines().count();
    let banner_y: f64 = 0.0;
    let head_y: f64 = (banner_height + 1) as f64;
    let tail_y: f64 = (banner_height + 1 + license_head_height + 1) as f64;

    let total_scene_h = lines.len() as f64;
    let total_scene_w = lines.iter().map(|l| l.len()).max().unwrap_or(80) as f64;

    Scene::new()
        // Banner — scroll + blended plasma/rainbow
        .block(&banner_text, FadeEnvelope::new(
            Scroll::new(&banner_text, storm.clone(), ScrollDirection::Left, Easing::Elastic(0.15), fps * 3, 0)
                .with_color(Blend::new(
                    Plasma::new(&banner_text, storm.clone(), 42.0)
                        .with_y_offset(banner_y)
                        .with_scene_size(total_scene_w, total_scene_h),
                    Rainbow::new(&banner_text),
                    BlendMode::Screen,
                )),
            fg, fps, fps * 2, total, Easing::EaseOut, Easing::EaseInOut,
        ))
        .line(Line::blank())
        // License head — plasma
        .block(&license_head_text, FadeEnvelope::new(
            Scroll::new(&license_head_text, fire.clone(), ScrollDirection::Left, Easing::Elastic(0.25), fps * 2, 2)
                .with_color(Plasma::new(&license_head_text, fire.clone(), 42.0)
                    .with_y_offset(head_y)
                    .with_scene_size(total_scene_w, total_scene_h)),
            fg, fps, fps * 2, total, Easing::EaseOut, Easing::EaseInOut,
        ))
        .line(Line::blank())
        // Last alinea — blended plasma + glow, overlay mode
        .block(&license_tail_text, FadeEnvelope::new(
            Scroll::new(&license_tail_text, fire.clone(), ScrollDirection::Right, Easing::Elastic(0.25), fps * 2, 2)
                .with_color(Blend::new(
                    Plasma::new(&license_tail_text, fire.clone(), 42.0)
                        .with_y_offset(tail_y)
                        .with_scene_size(total_scene_w, total_scene_h),
                    Glow::new(&license_tail_text, storm.clone()),
                    BlendMode::Overlay,
                )),
            fg, fps, fps * 2, total, Easing::EaseOut, Easing::EaseInOut,
        ))
        .run(Duration::from_secs(15))
        .await;
}
