//! Where everything sits once the wordmark is at the top, top to bottom:
//! wordmark (with as much DYCP swing as the screen leaves room for),
//! tagline, panel, block heading, pitch scroller. All in pixel rows
//! unless noted.

use super::panel::PANEL_H;
use super::tiny;
use super::wordmark;
use super::SCROLLER_Y;

/// Topmost pixel row anything may reach (row 0 is the border).
pub const TOP_MARGIN: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// Wordmark letter scale.
    pub scale: usize,
    /// DYCP swing at the top, in pixels each way.
    pub top_swing: i64,
    /// Wordmark glyph top at the top (before swing).
    pub logo_y: i64,
    /// Tagline glyph top when the wordmark is at the top.
    pub tagline_y: i64,
    /// Panel interior top, in terminal rows.
    pub panel_y: i64,
    /// Block heading glyph top.
    pub heading_y: i64,
}

impl Layout {
    /// The layout for a screen `screen_w` columns wide: the biggest top
    /// swing (up to the wordmark's full swing) that still leaves the
    /// heading clear of the scroller.
    pub fn for_width(screen_w: usize) -> Self {
        let scale = wordmark::scale_for(screen_w);
        let full = wordmark::full_swing(scale);
        (0..=full)
            .rev()
            .map(|s| Self::with_swing(scale, s))
            .find(|l| l.heading_y + tiny::STAMPED_HEIGHT <= scroller_top())
            .unwrap_or_else(|| Self::with_swing(scale, 0))
    }

    fn with_swing(scale: usize, s: i64) -> Self {
        let logo_y = TOP_MARGIN + s + 1; // swing up, outline
        let tagline_y = logo_y + wordmark::height(scale) + 2 + s + 2; // extrusion, swing down, outline
        let tagline_last = tagline_y + tiny::STAMPED_HEIGHT - 1;
        let panel_y = (tagline_last + 3) / 2; // outline, then the interior on a whole row
        let panel_last = (panel_y + PANEL_H as i64) * 2 + 2; // outline, extrusion
        let heading_y = panel_last + 2;
        Self { scale, top_swing: s, logo_y, tagline_y, panel_y, heading_y }
    }

    /// Lowest pixel row the wordmark reaches at the top.
    #[cfg(test)]
    pub fn logo_last(&self) -> i64 {
        self.logo_y + wordmark::height(self.scale) + 2 + self.top_swing
    }
}

/// Highest pixel the pitch scroller reaches: wave peak, outline.
fn scroller_top() -> i64 {
    SCROLLER_Y - 4 - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_stacks_without_touching() {
        for w in [80, 112, 120, 160, 200, 300] {
            let l = Layout::for_width(w);
            let panel_top = l.panel_y * 2 - 1;
            let panel_last = (l.panel_y + PANEL_H as i64) * 2 + 2;
            assert!(l.logo_y - l.top_swing - 1 >= TOP_MARGIN, "{w}: {l:?}");
            assert!(l.logo_last() < l.tagline_y - 1, "{w}: {l:?}");
            assert!(l.tagline_y + tiny::STAMPED_HEIGHT - 1 < panel_top, "{w}: {l:?}");
            assert!(panel_last < l.heading_y - 1, "{w}: {l:?}");
            assert!(l.heading_y + tiny::STAMPED_HEIGHT - 1 < scroller_top(), "{w}: {l:?}");
        }
    }

    #[test]
    fn narrow_screens_get_a_bigger_bounce() {
        // 1× letters leave room: well over the old calm ±2 px.
        assert!(Layout::for_width(112).top_swing >= 4);
        assert!(Layout::for_width(112).top_swing > Layout::for_width(200).top_swing);
    }
}
