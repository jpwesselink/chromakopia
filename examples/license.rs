use chromakopia::animate;
use chromakopia::animate::{Easing, FadeKind, FadeTarget, ScrollDirection, TimeRange};

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
    let full_text = chromakopia::pad(LICENSE);
    let line_count = full_text.lines().count();

    let fps = 30;
    let frames_per_line = 90;
    let stagger = 1;
    // Total time = (last line start + animation duration) / fps
    let total_secs = ((line_count - 1) * stagger + frames_per_line) as f64 / fps as f64;

    animate::Sequence::new(&full_text)
        .effect(
            TimeRange::new(0.0, total_secs), fps as u64,
            animate::scroll_staggered_effect(
                ScrollDirection::Left,
                Easing::Elastic(0.2),
                chromakopia::gradient(&["#ffffff", "#ffffff"]),
                frames_per_line,
                stagger,
            ),
        )
        .fade(
            TimeRange::new(total_secs - 0.5, total_secs),
            FadeKind::FadeTo(FadeTarget::Foreground),
            Easing::Linear,
        )
        .run(1.0)
        .await;
}
