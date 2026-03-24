use chromakopia::{animate, presets};
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
    let scroll_secs = ((line_count - 1) * stagger + frames_per_line) as f64 / fps as f64;
    let plasma_end = scroll_secs + 5.0;

    animate::Sequence::new(&full_text)
        // Slide in with elastic
        .effect(
            TimeRange::new(0.0, scroll_secs), fps as u64,
            animate::scroll_staggered_effect(
                ScrollDirection::Left,
                Easing::Elastic(0.25),
                presets::storm(),
                frames_per_line,
                stagger,
            ),
        )
        // Plasma takes over once text is in place
        .effect(
            TimeRange::new(scroll_secs, plasma_end), 30,
            animate::plasma_gradient_effect(presets::storm()),
        )
        // Fade to foreground to settle
        .fade(
            TimeRange::new(plasma_end - 2.0, plasma_end),
            FadeKind::FadeTo(FadeTarget::Foreground),
            Easing::EaseInOut,
        )
        .run(1.0)
        .await;
}
