//! Where each part of the screen goes.
//!
//! tomat draws a fixed block of [`HEIGHT`] rows under the shell prompt,
//! indented by [`MARGIN`]. The clock sits on the left with a side column
//! beside it; in a narrow pane the side column folds into one line above
//! the clock instead. See `view.rs` for a picture.

use ratatui::layout::{Constraint, Layout, Rect};

use super::digits;

/// Rows tomat occupies under the prompt.
pub const HEIGHT: u16 = 10;

/// Width of the side column. It is shown only when it gets all of it;
/// in a narrower pane it folds into one line above the clock.
pub const SIDE_WIDTH: u16 = 21;

/// Blank columns left of the content, so it reads as indented output.
const MARGIN: u16 = 2;

/// Columns between the clock and the side column.
const GUTTER: u16 = 4;

/// The areas of one frame. `side` is `None` when the pane is too narrow for
/// the side column; the label and first caption line then go on the `fold`
/// row above the clock instead.
#[derive(Debug)]
pub struct Areas {
    /// The big clock.
    pub clock: Rect,
    /// The column beside the clock: label, captions, tally.
    pub side: Option<Rect>,
    /// The row above the clock, used only when `side` is `None`.
    pub fold: Rect,
    /// The row under the clock: progress bar or presets.
    pub bar: Rect,
    /// The bottom row: key hints or a notice.
    pub footer: Rect,
}

/// Splits `area` (the inline viewport) for a clock `clock_cells` wide.
#[must_use]
pub fn areas(area: Rect, clock_cells: usize) -> Areas {
    let clock_width = u16::try_from(clock_cells).unwrap_or(u16::MAX);
    let content = Rect {
        x: area.x + MARGIN.min(area.width),
        width: area
            .width
            .saturating_sub(MARGIN * 2)
            .min(clock_width.saturating_add(GUTTER + SIDE_WIDTH)),
        ..area
    };
    let [fold, top, _, bar, _, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(u16::try_from(digits::HEIGHT).unwrap_or(5)),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(content);

    let side_width = top.width.saturating_sub(clock_width.saturating_add(GUTTER));
    let (clock, side) = if side_width >= SIDE_WIDTH {
        let [clock, _, side] = Layout::horizontal([
            Constraint::Length(clock_width),
            Constraint::Length(GUTTER),
            Constraint::Fill(1),
        ])
        .areas(top);
        (clock, Some(side))
    } else {
        (top, None)
    };
    Areas {
        clock,
        side,
        fold,
        bar,
        footer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_column_folds_away_in_a_narrow_pane() {
        let wide = areas(Rect::new(0, 0, 80, HEIGHT), 33);
        assert!(wide.side.is_some_and(|side| side.width == SIDE_WIDTH));
        assert!(areas(Rect::new(0, 0, 50, HEIGHT), 33).side.is_none());
    }

    #[test]
    fn a_longer_clock_gets_a_wider_clock_area() {
        let wide = areas(Rect::new(0, 0, 100, HEIGHT), digits::width("120:00"));
        assert_eq!(usize::from(wide.clock.width), digits::width("120:00"));
        assert!(wide.side.is_some());
    }
}
