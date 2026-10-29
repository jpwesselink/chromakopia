//! Figlet `.flf` fonts for the panel names.

// ── Minimal figlet .flf loader ──────────────────────────────────────
// Full-width layout only (no smushing/kerning): every letter stands on
// its own. Header: `flf2a$ height baseline maxlen oldlayout comments`;
// the hardblank is the last char of the signature. Then 95 glyphs
// (ASCII 32–126), `height` lines each, trailing endmark chars stripped.

pub struct FigletFont {
    pub height: usize,
    glyphs: Vec<Vec<String>>, // index = ascii − 32
}

impl FigletFont {
    pub fn parse(src: &str) -> Self {
        let mut lines = src.lines();
        let header = lines.next().expect("empty .flf");
        let mut fields = header.split_whitespace();
        let sig = fields.next().expect(".flf signature");
        let hardblank = sig.chars().last().expect(".flf hardblank");
        let height: usize = fields.next().and_then(|f| f.parse().ok()).expect(".flf height");
        let _baseline = fields.next();
        let _max_len = fields.next();
        let _old_layout = fields.next();
        let comments: usize = fields.next().and_then(|f| f.parse().ok()).expect(".flf comments");
        for _ in 0..comments {
            lines.next();
        }

        let mut glyphs = Vec::with_capacity(95);
        'glyphs: while glyphs.len() < 95 {
            let mut rows = Vec::with_capacity(height);
            for _ in 0..height {
                let Some(raw) = lines.next() else { break 'glyphs };
                let line = raw.trim_end_matches('\r');
                let endmark = line.chars().last().unwrap_or('@');
                rows.push(line.trim_end_matches(endmark).replace(hardblank, " "));
            }
            glyphs.push(rows);
        }
        assert_eq!(glyphs.len(), 95, ".flf ended before ASCII 32-126 glyphs");
        Self { height, glyphs }
    }

    fn glyph(&self, ch: char) -> &Vec<String> {
        let idx = (ch as usize)
            .checked_sub(32)
            .filter(|&i| i < 95)
            .unwrap_or(0); // unknown chars render as space
        &self.glyphs[idx]
    }

    fn glyph_width(glyph: &[String]) -> usize {
        glyph.iter().map(|r| r.chars().count()).max().unwrap_or(0)
    }

    /// Render `text` into `height` rows, full-width layout.
    pub fn render(&self, text: &str) -> Vec<String> {
        let mut rows = vec![String::new(); self.height];
        for ch in text.chars() {
            let glyph = self.glyph(ch);
            let w = Self::glyph_width(glyph);
            for (i, row) in rows.iter_mut().enumerate() {
                row.push_str(&glyph[i]);
                for _ in glyph[i].chars().count()..w {
                    row.push(' ');
                }
            }
        }
        rows
    }

    /// Rendered width of `text` in cells.
    pub fn width(&self, text: &str) -> usize {
        text.chars().map(|ch| Self::glyph_width(self.glyph(ch))).sum()
    }
}

const COLOSSAL_FLF: &str = include_str!("../fonts/colossal.flf");
const BASIC_FLF: &str = include_str!("../fonts/basic.flf");

pub fn colossal() -> FigletFont {
    FigletFont::parse(COLOSSAL_FLF)
}

pub fn basic() -> FigletFont {
    FigletFont::parse(BASIC_FLF)
}

/// `text` in Colossal when it fits in `max_w` columns, else in Basic,
/// else as plain text (cut to `max_w`).
pub fn fit(text: &str, max_w: usize) -> Vec<String> {
    for font in [colossal(), basic()] {
        if font.width(text) <= max_w {
            // Drop blank rows (descender space) so the ink centres.
            let mut rows = font.render(text);
            while rows.last().is_some_and(|r| r.trim().is_empty()) {
                rows.pop();
            }
            let top = rows.iter().take_while(|r| r.trim().is_empty()).count();
            return rows.split_off(top);
        }
    }
    vec![text.chars().take(max_w).collect()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_load_with_expected_heights() {
        assert_eq!(colossal().height, 11);
        assert_eq!(basic().height, 8);
    }

    #[test]
    fn render_rows_and_width_add_up() {
        let f = colossal();
        let rows = f.render("AB");
        assert_eq!(rows.len(), 11);
        assert_eq!(f.width("AB"), f.width("A") + f.width("B"));
        assert!(rows.iter().all(|r| r.chars().count() == f.width("AB")));
    }

    #[test]
    fn fit_trims_blank_rows_so_names_centre() {
        let rows = fit("PLASMA", 104);
        assert!(rows.first().unwrap().trim().len() > 0);
        assert!(rows.last().unwrap().trim().len() > 0);
        assert_eq!(rows.len(), 8);
    }

    #[test]
    fn fit_always_fits() {
        use super::super::stage::{storyboard, StepKind};
        for st in storyboard() {
            let StepKind::Panel { name, .. } = st.kind else { continue };
            for max_w in [12, 20, 40, 68] {
                let rows = fit(name, max_w);
                assert!(rows.iter().all(|r| r.chars().count() <= max_w), "{name} at {max_w}: {rows:?}");
            }
        }
        assert_eq!(fit("TRANSITION", 20), vec!["TRANSITION".to_string()]);
    }

    #[test]
    fn fit_falls_back_to_basic() {
        let wide = colossal().width("VECTORFIELD");
        let width = |rows: Vec<String>| rows[0].chars().count();
        assert_eq!(width(fit("VECTORFIELD", wide)), wide);
        assert_eq!(width(fit("VECTORFIELD", wide - 1)), basic().width("VECTORFIELD"));
    }
}
