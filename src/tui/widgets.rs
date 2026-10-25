//! The small pieces each screen is built from: the clock, the progress bar,
//! the presets row, the tally of today's sessions and the key hints.
//!
//! Each returns plain `Line`s, so they are tested without a terminal.

use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::digits;
use super::theme::{self, FG, GRAY, ORANGE, SPARK, SURFACE};
use crate::pomodoro::{Plan, compact};

/// Most tally dots shown; past this a `+` stands in for the rest. Six
/// spaced dots and a two-digit count fill the side column exactly; bigger
/// counts get fewer dots so the line still fits.
const MAX_DOTS: usize = 6;

/// `MM:SS`, rounding seconds up so a fresh 30-minute timer reads `30:00`
/// and `00:00` appears only when time is really up.
#[must_use]
pub fn clock_text(remaining: Duration) -> String {
    // Nanoseconds, so even a sliver of a second still shows as `00:01`.
    let secs = remaining.as_nanos().div_ceil(1_000_000_000);
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// The clock as big digits when they fit in `area`, otherwise one plain line.
#[must_use]
pub fn clock(text: &str, area: Rect, style: Style) -> Vec<Line<'static>> {
    if usize::from(area.width) >= digits::width(text) {
        digits::render(text)
            .into_iter()
            .map(|row| Line::styled(row, style))
            .collect()
    } else {
        // On the middle row, so the plain clock sits where the big one would.
        let mut lines = vec![Line::default(); digits::HEIGHT / 2];
        lines.push(Line::styled(
            text.to_owned(),
            style.add_modifier(Modifier::BOLD),
        ));
        lines
    }
}

/// How long each animation step lasts. Each step moves the dots half a
/// cell backward, so they stream off the front like a trail.
pub const ANIMATION_STEP: Duration = Duration::from_millis(200);

/// A 4×4 Bayer matrix: the order dots light up in as the layer gets
/// denser. Over braille's 2×4 dots per cell it gives sixteen even steps
/// and an evenly scattered sparkle.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Braille dot bits by row (top to bottom) and column (left, right), in
/// Unicode's dot numbering.
const DOTS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Share of the filled part, back from its front edge, that carries dots...
const FRONT: f64 = 0.45;
/// ...but never more than this share of the whole bar.
const MAX_FRONT: f64 = 0.3;
/// Dot density at the very front, where it is highest.
const PEAK: f64 = 0.95;
/// How sharply the density rises toward the front (1 would be linear).
const CURVE: f64 = 1.3;

/// A full-height progress bar in two layers, then the percentage in a
/// tinted chip (six cells of `width` go to the chip):
///
/// 1. the done part, solid `color`, on a quiet track;
/// 2. a layer of cream dots over its front, densest at the leading edge and
///    thinning out behind it, like the sparkle trail of something flying.
///
/// `step` animates the dots (see [`ANIMATION_STEP`]). The same step always
/// draws the same bar, so a paused timer shows a still one.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "progress is clamped to 0..=1 and widths are terminal-sized"
)]
#[must_use]
pub fn progress_bar(progress: f64, width: u16, color: Color, step: usize) -> Line<'static> {
    let progress = progress.clamp(0.0, 1.0);
    let cells = usize::from(width).saturating_sub(6);
    let filled = ((progress * cells as f64).round() as usize).min(cells);
    Line::from(vec![
        // The dots are the foreground and the bar is the background, so
        // the two layers share each cell.
        Span::styled(
            sparkles(filled, cells, step),
            Style::new().fg(SPARK).bg(color),
        ),
        Span::styled(" ".repeat(cells - filled), Style::new().bg(SURFACE)),
        Span::raw(" "),
        Span::styled(
            format!("{:>3}% ", (progress * 100.0).floor() as u8),
            Style::new()
                .fg(color)
                .bg(theme::tint(color))
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

/// The dot layer over the `filled` cells of a `cells`-wide bar, as braille.
fn sparkles(filled: usize, cells: usize, step: usize) -> String {
    sparkle_dots(filled, cells, step)
        .into_iter()
        .map(|dots| match dots {
            0 => ' ',
            _ => char::from_u32(0x2800 + u32::from(dots)).unwrap_or(' '),
        })
        .collect()
}

/// Which of each filled cell's eight braille dots are lit (see [`DOTS`]).
#[expect(
    clippy::cast_precision_loss,
    reason = "terminal widths are far below f64's exact integer range"
)]
fn sparkle_dots(filled: usize, cells: usize, step: usize) -> Vec<u8> {
    let front = (filled as f64 * FRONT).min(cells as f64 * MAX_FRONT);
    let start = filled as f64 - front;
    (0..filled)
        .map(|i| {
            let x = i as f64 + 0.5;
            let density = if x < start {
                0.0
            } else {
                ((x - start) / front).powf(CURVE) * PEAK
            };
            let mut lit = 0;
            for (row, bits) in DOTS.iter().enumerate() {
                for (column, bit) in bits.iter().enumerate() {
                    // Reading the matrix one column further on each step
                    // moves the pattern backward, away from the front.
                    let rank = BAYER[row][(2 * i + column + step) % 4];
                    if density > (f64::from(rank) + 0.5) / 16.0 {
                        lit |= bit;
                    }
                }
            }
            lit
        })
        .collect()
}

/// A status chip on the raised surface: a colored `mark`, then `text`.
#[must_use]
pub fn status(text: &str, mark: char, mark_color: Color, text_color: Color) -> Line<'static> {
    let chip = Style::new().bg(SURFACE);
    Line::from(vec![
        Span::styled(" ", chip),
        Span::styled(mark.to_string(), chip.fg(mark_color)),
        Span::styled(
            format!(" {text} "),
            chip.fg(text_color).add_modifier(Modifier::BOLD),
        ),
    ])
}

/// The presets in a row, e.g. ` 15/5   30/5   60/10 `; the selected one is
/// a solid block, like the current window in tmux. When they don't all fit
/// in `width`, the row starts late enough to keep the selected one in view,
/// and presets are left out rather than cut.
#[must_use]
pub fn presets(plans: &[Plan], selected: usize, width: u16) -> Line<'static> {
    const GAP: usize = 2;
    let labels: Vec<String> = plans
        .iter()
        .map(|plan| format!(" {}/{} ", compact(plan.focus), compact(plan.rest)))
        .collect();
    let width = usize::from(width);
    let span = |range: &[String]| -> usize {
        range.iter().map(|l| l.chars().count()).sum::<usize>() + GAP * range.len().saturating_sub(1)
    };
    let selected = selected.min(labels.len().saturating_sub(1));
    // The first preset to show: the earliest that still leaves room for
    // everything up to and including the selected one.
    let first = (0..=selected)
        .find(|&start| span(&labels[start..=selected]) <= width)
        .unwrap_or(selected);

    let mut spans = Vec::new();
    let mut used = 0;
    for (i, label) in labels.into_iter().enumerate().skip(first) {
        let gap = if i == first { 0 } else { GAP };
        used += gap + label.chars().count();
        if used > width {
            break;
        }
        spans.push(Span::raw(" ".repeat(gap)));
        let style = if i == selected {
            theme::block(FG)
        } else {
            Style::new().fg(FG)
        };
        spans.push(Span::styled(label, style));
    }
    Line::from(spans)
}

/// The `i` chip that shows and hides the key list: quiet while the list is
/// hidden, lit while it is shown.
#[must_use]
pub fn keys_chip(shown: bool) -> Line<'static> {
    let style = if shown {
        theme::block(FG)
    } else {
        Style::new()
            .fg(GRAY)
            .bg(SURFACE)
            .add_modifier(Modifier::BOLD)
    };
    Line::styled(" i ", style)
}

/// Today's finished focus sessions: one orange dot each, then the number,
/// in at most `width` cells. Past [`MAX_DOTS`] (or what fits), a `+` stands
/// in for the rest.
#[must_use]
pub fn tally(today: usize, width: u16) -> Line<'static> {
    let count = format!("{today} today");
    let mut shown = today.min(MAX_DOTS);
    let line = |shown: usize| {
        let mut dots = vec!["●"; shown].join(" ");
        if shown < today {
            dots.push('+');
        }
        if !dots.is_empty() {
            dots.push(' ');
        }
        dots
    };
    while shown > 0 && line(shown).chars().count() + count.len() > usize::from(width) {
        shown -= 1;
    }
    Line::from(vec![
        Span::styled(line(shown), Style::new().fg(ORANGE)),
        Span::styled(count, Style::new().fg(GRAY)),
    ])
}

/// The key hints, as many as fit in `width`. When some must go, they go
/// from the end, just before the last: the main action (first) and quit
/// (last) always stay.
#[must_use]
pub fn hints(pairs: &[(&'static str, &'static str)], width: u16) -> Line<'static> {
    const SEPARATOR: &str = "   ";
    let cost = |(k, label): &(&str, &str)| k.chars().count() + 1 + label.chars().count();
    let total = |shown: &[(&str, &str)]| {
        shown.iter().map(cost).sum::<usize>() + SEPARATOR.len() * shown.len().saturating_sub(1)
    };
    let mut shown = pairs.to_vec();
    while shown.len() > 2 && total(&shown) > usize::from(width) {
        shown.remove(shown.len() - 2);
    }

    let mut spans = Vec::new();
    for (i, (k, label)) in shown.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(SEPARATOR));
        }
        spans.push(Span::styled(k, theme::key()));
        spans.push(Span::styled(format!(" {label}"), Style::new().fg(GRAY)));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::super::layout::SIDE_WIDTH;
    use super::*;

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_clock_rounds_up_to_the_next_second() {
        assert_eq!(clock_text(Duration::from_micros(400)), "00:01");
        assert_eq!(clock_text(Duration::from_secs(1500)), "25:00");
        assert_eq!(clock_text(Duration::from_millis(1_499_001)), "25:00");
        assert_eq!(clock_text(Duration::from_millis(1)), "00:01");
        assert_eq!(clock_text(Duration::ZERO), "00:00");
        assert_eq!(clock_text(Duration::from_secs(100 * 60)), "100:00");
    }

    #[test]
    fn the_progress_bar_keeps_its_width_from_start_to_end() {
        for progress in [0.0, 0.33, 1.0, 1.5] {
            assert_eq!(progress_bar(progress, 58, ORANGE, 0).width(), 58);
        }
    }

    /// Dots lit in each cell of a layer.
    fn counts(filled: usize, step: usize) -> Vec<u32> {
        sparkle_dots(filled, 52, step)
            .iter()
            .map(|m| m.count_ones())
            .collect()
    }

    #[test]
    fn the_dot_layer_covers_only_the_front_of_the_bar() {
        assert_eq!(sparkles(0, 52, 0), "");
        let half = counts(26, 0);
        assert_eq!(half.len(), 26, "one layer cell per filled cell");
        // The back of the bar is plain solid color.
        assert!(half[..14].iter().all(|&n| n == 0), "{half:?}");
        assert!(half[14..].iter().any(|&n| n > 0), "{half:?}");
    }

    #[test]
    fn the_dots_get_denser_toward_the_front() {
        for step in 0..4 {
            let dots = counts(40, step);
            let (back, front) = dots[28..].split_at(6);
            assert!(
                front.iter().sum::<u32>() > back.iter().sum::<u32>(),
                "step {step}: {dots:?}"
            );
        }
    }

    #[test]
    fn the_dots_move_with_each_step_and_repeat_every_four() {
        let frames: Vec<_> = (0..5).map(|step| sparkles(30, 52, step)).collect();
        assert_ne!(frames[0], frames[1]);
        assert_eq!(frames[0], frames[4]);
    }

    #[test]
    fn the_tally_shows_a_dot_per_session_up_to_a_limit() {
        assert_eq!(text(&tally(0, 21)), "0 today");
        assert_eq!(text(&tally(3, 21)), "● ● ● 3 today");
        assert_eq!(text(&tally(15, 21)), "● ● ● ● ● ●+ 15 today");
    }

    #[test]
    fn a_two_digit_tally_fits_the_narrowest_side_column() {
        assert!(tally(99, SIDE_WIDTH).width() <= usize::from(SIDE_WIDTH));
    }

    #[test]
    fn a_big_tally_drops_dots_to_fit() {
        let line = tally(100, SIDE_WIDTH);
        assert!(line.width() <= usize::from(SIDE_WIDTH));
        assert!(text(&line).ends_with("+ 100 today"), "{:?}", text(&line));
    }

    #[test]
    fn presets_that_do_not_fit_are_left_out_rather_than_cut() {
        let plans = Plan::presets();
        assert_eq!(presets(&plans, 0, 58).width(), 23);
        assert_eq!(presets(&plans, 0, 12).width(), 6, "only the first fits");
    }

    #[test]
    fn the_selected_preset_stays_in_view_in_a_narrow_row() {
        let plans = Plan::presets();
        let row = text(&presets(&plans, 2, 16));
        assert!(row.contains("60/10"), "{row:?}");
        assert!(!row.contains("15/5"), "{row:?}");
    }

    #[test]
    fn hints_that_do_not_fit_drop_from_the_end_keeping_quit() {
        let pairs = [
            ("space", "pause"),
            ("s", "skip"),
            ("r", "reset"),
            ("q", "quit"),
        ];
        assert_eq!(
            text(&hints(&pairs, 80)),
            "space pause   s skip   r reset   q quit"
        );
        assert_eq!(text(&hints(&pairs, 30)), "space pause   s skip   q quit");
    }
}
