//! A 5-row line font for the big clock.
//!
//! Vertical strokes are one full block (`█`) wide and horizontal strokes are
//! half a row (`▀` / `▄`) tall. A terminal cell is about twice as tall as it
//! is wide, so both strokes come out the same thickness, which makes clean,
//! even digits at twice the vertical resolution of whole blocks.

/// Rows in every glyph.
pub const HEIGHT: usize = 5;

/// Blank columns between glyphs.
const GAP: usize = 2;

/// The rows of `c`, or `None` for characters the font doesn't have. Every
/// digit is 6 columns wide, so the clock doesn't shift as digits change.
fn glyph(c: char) -> Option<[&'static str; HEIGHT]> {
    Some(match c {
        '0' => ["█▀▀▀▀█", "█    █", "█    █", "█    █", "█▄▄▄▄█"],
        '1' => ["  ▀█  ", "   █  ", "   █  ", "   █  ", "  ▄█▄ "],
        '2' => ["▀▀▀▀▀█", "     █", "█▀▀▀▀▀", "█     ", "█▄▄▄▄▄"],
        '3' => ["▀▀▀▀▀█", "     █", " ▀▀▀▀█", "     █", "▄▄▄▄▄█"],
        '4' => ["█    █", "█    █", "▀▀▀▀▀█", "     █", "     █"],
        '5' => ["█▀▀▀▀▀", "█     ", "▀▀▀▀▀█", "     █", "▄▄▄▄▄█"],
        '6' => ["█▀▀▀▀▀", "█     ", "█▀▀▀▀█", "█    █", "█▄▄▄▄█"],
        '7' => ["▀▀▀▀▀█", "     █", "     █", "     █", "     █"],
        '8' => ["█▀▀▀▀█", "█    █", "█▀▀▀▀█", "█    █", "█▄▄▄▄█"],
        '9' => ["█▀▀▀▀█", "█    █", "▀▀▀▀▀█", "     █", "▄▄▄▄▄█"],
        ':' => [" ", "▄", " ", "▀", " "],
        _ => return None,
    })
}

/// Renders `text` as five rows. Characters the font doesn't have are skipped.
#[must_use]
pub fn render(text: &str) -> [String; HEIGHT] {
    let mut rows: [String; HEIGHT] = Default::default();
    for (i, glyph) in text.chars().filter_map(glyph).enumerate() {
        for (row, part) in rows.iter_mut().zip(glyph) {
            if i > 0 {
                row.push_str(&" ".repeat(GAP));
            }
            row.push_str(part);
        }
    }
    rows
}

/// Width in terminal cells of `render(text)`.
#[must_use]
pub fn width(text: &str) -> usize {
    let glyphs: Vec<_> = text.chars().filter_map(glyph).collect();
    let cells: usize = glyphs.iter().map(|g| g[0].chars().count()).sum();
    cells + GAP * glyphs.len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_row_has_the_same_width() {
        for c in "0123456789:".chars() {
            let rows = glyph(c).expect("font has it");
            let w = rows[0].chars().count();
            assert!(rows.iter().all(|r| r.chars().count() == w), "{c:?}");
        }
    }

    #[test]
    fn rendered_rows_have_the_reported_width() {
        for text in ["25:00", "00:07", "105:59", "1"] {
            for row in &render(text) {
                assert_eq!(row.chars().count(), width(text), "{text:?}");
            }
        }
    }

    #[test]
    fn a_clock_fits_in_thirty_three_cells() {
        // 4 digits of 6 + a colon of 1 + 4 gaps of 2.
        assert_eq!(width("25:00"), 33);
    }

    #[test]
    fn unknown_characters_are_skipped() {
        assert_eq!(render("1x"), render("1"));
    }
}
