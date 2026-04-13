use chromakopia::animate::*;
use chromakopia::{pad, presets};


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
    let fire = chromakopia::gradient(&[
        "#ffffff", &bg_hex, "#ff69b4", "#00cccc", "#fffacd", "#8b4513", &bg_hex, "#ffffff",
    ])
    .palette(256);
    // Split license into three parts:
    // - top: MIT License, Copyright, Permission...conditions
    // - mid: "The above copyright notice..." paragraph
    // - tail: "THE SOFTWARE IS PROVIDED..." paragraph
    let alineas: Vec<&str> = LICENSE.split("\n\n").collect();
    let license_top = alineas[..alineas.len() - 2].join("\n\n");
    let license_mid = alineas[alineas.len() - 2];
    let license_tail = alineas[alineas.len() - 1];

    let full = pad(&format!(
        "{}\n\n{}\n\n{}\n\n{}",
        BANNER, license_top, license_mid, license_tail
    ));
    let lines: Vec<&str> = full.lines().collect();

    let banner_height = BANNER.lines().count();
    let top_height = license_top.lines().count();
    let mid_height = license_mid.lines().count();
    let tail_height = license_tail.lines().count();

    let banner_text: String = lines[..banner_height].join("\n");
    let top_start = banner_height + 1;
    let license_top_text: String = lines[top_start..top_start + top_height].join("\n");
    let mid_start = top_start + top_height + 1;
    let license_mid_text: String = lines[mid_start..mid_start + mid_height].join("\n");
    let tail_start = mid_start + mid_height + 1;
    let license_tail_text: String = lines[tail_start..tail_start + tail_height].join("\n");

    // Y offsets for continuous plasma field
    let banner_y: f64 = 0.0;
    let top_y: f64 = (banner_height + 1) as f64;
    let mid_y: f64 = (banner_height + 1 + top_height + 1) as f64;
    let tail_y: f64 = (banner_height + 1 + top_height + 1 + mid_height + 1) as f64;

    let total_scene_h = lines.len() as f64;
    let total_scene_w = lines.iter().map(|l| l.len()).max().unwrap_or(80) as f64;

    let scene = Scene::new()
        // Banner — scroll + blended plasma/rainbow
        .add(FadeEnvelope::new(
            Scroll::new(&banner_text)
                .palette(storm.clone())
                .direction(ScrollDirection::Left)
                .easing(Easing::Elastic(0.15))
                .duration(3.0)
                .color(Blend::new(
                    Plasma::new().palette(storm.clone()).seed(42.0)
                        .y_offset(banner_y)
                        .scene_size(total_scene_w, total_scene_h),
                    Rainbow::new(),
                    BlendMode::Screen,
                )),
        ).total(15.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(fg))
        .blank()
        // License top — plasma with fire palette
        .add(FadeEnvelope::new(
            Scroll::new(&license_top_text)
                .palette(fire.clone())
                .direction(ScrollDirection::Left)
                .easing(Easing::Elastic(0.25))
                .duration(2.0)
                .stagger(2)
                .color(Blend::new(
                    Plasma::new().palette(fire.clone()).seed(42.0)
                        .y_offset(top_y)
                        .scene_size(total_scene_w, total_scene_h),
                    Radar::new(),
                    BlendMode::Screen,
                )),
        ).total(15.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(fg))
        .blank()
        // "The above copyright notice..." — shifted palette
        .add(FadeEnvelope::new(
            Scroll::new(&license_mid_text)
                .palette(fire.clone())
                .direction(ScrollDirection::Left)
                .easing(Easing::Elastic(0.25))
                .duration(2.0)
                .stagger(2)
                .color(Blend::new(
                    Plasma::new().palette(fire.clone()).seed(42.0)
                        .y_offset(mid_y)
                        .scene_size(total_scene_w, total_scene_h),
                    Radar::reversed(),
                    BlendMode::Screen,
                )),
        ).total(15.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(fg))
        .blank()
        // Last alinea — blended plasma + radar Screen
        .add(FadeEnvelope::new(
            Scroll::new(&license_tail_text)
                .palette(fire.clone())
                .direction(ScrollDirection::Right)
                .easing(Easing::Elastic(0.25))
                .duration(2.0)
                .stagger(2)
                .color(Blend::new(
                    Plasma::new().palette(fire.clone()).seed(42.0)
                        .y_offset(tail_y)
                        .scene_size(total_scene_w, total_scene_h),
                    Radar::new(),
                    BlendMode::Screen,
                )),
        ).total(15.0).fade_in(1.0, Easing::EaseOut).fade_out(2.0, Easing::EaseInOut).from_color(fg))
    ;
    scene.run(15.0).await;
}
