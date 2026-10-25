//! What each screen shows, and drawing it into a frame.
//!
//! Every screen uses the same rows, so moving between the menu and a
//! session changes content, never layout:
//!
//! ```text
//!   █▀▀▀▀█  █    █    █▀▀▀▀▀  █▀▀▀▀█    ■ FOCUS         <- status chip
//!   █    █  █    █  ▄ █       █    █     ends 13:25     <- two caption lines
//!   ▀▀▀▀▀█  ▀▀▀▀▀█    ▀▀▀▀▀█  █▀▀▀▀█     / 30 min
//!   █          █    ▀      █  █    █
//!   █▄▄▄▄▄     █      ▄▄▄▄▄█  █▄▄▄▄█     ●● 2 today     <- tally
//!
//!   ██████████⠪⡫⡯⣿                             17%   i    <- bar or presets, keys chip
//!
//!   space pause   s skip   r reset   m menu   q quit        <- keys, once `i` is pressed
//! ```

use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::{self, FG, GRAY, ORANGE, RED};
use super::{digits, layout, widgets};
use crate::pomodoro::{App, Phase, Session, State, View, human};

/// What the screen shows, gathered from the app before drawing.
struct Screen {
    label: Line<'static>,
    lines: [Line<'static>; 2],
    clock: String,
    /// Cells to lay the clock out for: the widest it gets on this screen,
    /// so the side column never shifts as the clock counts down.
    clock_cells: usize,
    clock_color: Color,
    row: Row,
    hints: Vec<(&'static str, &'static str)>,
}

/// What goes on the row under the clock.
enum Row {
    /// The progress bar: fraction done, fill color and animation step.
    Bar(f64, Color, usize),
    /// The preset picker.
    Presets,
}

/// Draws the whole screen as it stands at `now`.
pub fn draw(frame: &mut Frame, app: &App, now: SystemTime) {
    let screen = match app.view() {
        View::Menu => menu_screen(app),
        View::Session(session) => session_screen(session, now),
    };
    let areas = layout::areas(frame.area(), screen.clock_cells);

    let clock = widgets::clock(
        &screen.clock,
        areas.clock,
        Style::new().fg(screen.clock_color),
    );
    frame.render_widget(Paragraph::new(clock), areas.clock);

    let [first, second] = screen.lines;
    if let Some(side) = areas.side {
        // Label and captions together; the tally apart, level with the
        // bottom of the clock.
        let column = vec![
            screen.label,
            first,
            second,
            Line::default(),
            widgets::tally(app.today(), side.width),
        ];
        frame.render_widget(Paragraph::new(column), side);
    } else {
        let mut spans = screen.label.spans;
        spans.push(Span::raw("  "));
        spans.extend(first.spans);
        frame.render_widget(Paragraph::new(Line::from(spans)), areas.fold);
    }

    // The `i` chip always sits at the end of the row, on every screen.
    let [row_area, _, chip_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(areas.bar);
    let row = match screen.row {
        Row::Bar(progress, color, step) => {
            widgets::progress_bar(progress, row_area.width, color, step)
        }
        Row::Presets => widgets::presets(app.plans(), app.selected(), row_area.width),
    };
    frame.render_widget(Paragraph::new(row), row_area);
    frame.render_widget(
        Paragraph::new(widgets::keys_chip(app.show_keys())),
        chip_area,
    );
    // The keys stay out of the way until `i` is pressed; the row itself
    // always stays, so toggling it moves nothing.
    let footer = match app.notice() {
        Some(notice) => Line::styled(format!("! {notice}"), Style::new().fg(RED)),
        None if app.show_keys() => widgets::hints(&screen.hints, areas.footer.width),
        None => Line::default(),
    };
    frame.render_widget(Paragraph::new(footer), areas.footer);
}

/// The menu: the selected preset's focus length on the clock, and the
/// presets in a row like tmux windows.
fn menu_screen(app: &App) -> Screen {
    let plan = app.plans()[app.selected()];
    let clock = widgets::clock_text(plan.focus);
    Screen {
        label: widgets::status("TOMAT", '■', ORANGE, FG),
        lines: [
            Line::styled(format!("{} focus", human(plan.focus)), Style::new().fg(FG)),
            Line::styled(format!("{} break", human(plan.rest)), Style::new().fg(GRAY)),
        ],
        clock_cells: digits::width(&clock),
        clock,
        clock_color: FG,
        row: Row::Presets,
        hints: vec![
            ("h/l", "choose"),
            (pick_keys(app.plans().len()), "pick"),
            ("enter", "start"),
            ("q", "quit"),
        ],
    }
}

/// The number keys that pick a preset, for `count` presets (keys go up
/// to 9).
fn pick_keys(count: usize) -> &'static str {
    const KEYS: [&str; 9] = ["1", "1-2", "1-3", "1-4", "1-5", "1-6", "1-7", "1-8", "1-9"];
    KEYS[count.clamp(1, KEYS.len()) - 1]
}

/// A session: phase label, countdown, progress bar.
fn session_screen(session: &Session, now: SystemTime) -> Screen {
    let timer = session.timer();
    let remaining = timer.remaining(now);
    let state = session.state();
    let accent = theme::phase_color(session.phase());
    // Every label is five letters, so nothing beside it ever shifts.
    let (label, clock_color, bar_color) = match state {
        State::Running => (session.phase().label(), FG, accent),
        State::Paused => ("PAUSE", GRAY, GRAY),
        State::Waiting => (session.phase().label(), GRAY, accent),
    };
    let hints = match state {
        State::Waiting => vec![
            ("enter", "start"),
            ("s", "skip"),
            ("m", "menu"),
            ("q", "quit"),
        ],
        // "pause " is padded to the width of "resume" so hints don't shift.
        State::Running | State::Paused => vec![
            (
                "space",
                if state == State::Paused {
                    "resume"
                } else {
                    "pause "
                },
            ),
            ("s", "skip"),
            ("r", "reset"),
            ("m", "menu"),
            ("q", "quit"),
        ],
    };
    let (mark, mark_color, text_color) = theme::status_marks(session);
    let full_length = widgets::clock_text(session.plan().length_of(session.phase()));
    Screen {
        label: widgets::status(label, mark, mark_color, text_color),
        lines: captions(session, remaining, now),
        clock: widgets::clock_text(remaining),
        clock_cells: digits::width(&full_length),
        clock_color,
        row: Row::Bar(timer.progress(now), bar_color, animation_step(session, now)),
        hints,
    }
}

/// Which animation step the bar's front is on. It follows the time
/// counted so far, so a paused timer keeps a still bar.
fn animation_step(session: &Session, now: SystemTime) -> usize {
    let elapsed = session.timer().elapsed(now).as_nanos();
    usize::try_from(elapsed / widgets::ANIMATION_STEP.as_nanos()).unwrap_or(0)
}

/// The two lines under the label: when the phase ends, or what to do next.
fn captions(session: &Session, remaining: Duration, now: SystemTime) -> [Line<'static>; 2] {
    let quiet = Style::new().fg(GRAY);
    let length = human(session.plan().length_of(session.phase()));
    match session.state() {
        State::Paused => [
            Line::from(vec![
                Span::styled("space", theme::key()),
                Span::styled(" to resume", quiet),
            ]),
            Line::default(),
        ],
        State::Waiting => {
            let status = match (session.is_after_finish(), session.phase()) {
                (true, Phase::Break) => "focus done",
                (true, Phase::Focus) => "break over",
                (false, _) => "ready",
            };
            [
                Line::styled(status, Style::new().fg(FG)),
                Line::from(vec![
                    Span::styled("enter", theme::key()),
                    Span::styled(format!(" for {length}"), quiet),
                ]),
            ]
        }
        State::Running => {
            let ends = DateTime::<Local>::from(now + remaining);
            [
                Line::styled(
                    format!("ends {}", ends.format("%H:%M")),
                    Style::new().fg(FG),
                ),
                Line::styled(format!("/ {length}"), quiet),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    //! Whole-screen render tests: draw into an in-memory terminal and
    //! compare the text, trailing spaces trimmed. Colors are checked by
    //! eye and by the theme tests; these catch layout regressions.

    use chrono::TimeZone;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::pomodoro::{Action, Plan};

    const MIN: Duration = Duration::from_secs(60);

    /// 13:00 local time on a summer day: a fixed start, so the "ends" times
    /// in these screens are the same in every time zone.
    fn t0() -> SystemTime {
        Local
            .with_ymd_and_hms(2026, 7, 15, 13, 0, 0)
            .single()
            .expect("an unambiguous local time")
            .into()
    }

    /// Draws `app` into a `width`-column terminal and returns its rows.
    fn render(app: &App, width: u16, now: SystemTime) -> Vec<String> {
        let mut terminal =
            Terminal::new(TestBackend::new(width, layout::HEIGHT)).expect("test terminal");
        terminal.draw(|frame| draw(frame, app, now)).expect("draw");
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                let row: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                row.trim_end().to_owned()
            })
            .collect()
    }

    #[test]
    fn the_menu_shows_the_selected_preset() {
        let t0 = t0();
        let mut app = App::new(Plan::presets(), false, 2);
        app.handle(Action::Next, t0);
        insta(
            &render(&app, 80, t0),
            &[
                "",
                "  ▀▀▀▀▀█  █▀▀▀▀█     █▀▀▀▀█  █▀▀▀▀█     ■ TOMAT",
                "       █  █    █  ▄  █    █  █    █    30 min focus",
                "   ▀▀▀▀█  █    █     █    █  █    █    5 min break",
                "       █  █    █  ▀  █    █  █    █",
                "  ▄▄▄▄▄█  █▄▄▄▄█     █▄▄▄▄█  █▄▄▄▄█    ● ● 2 today",
                "",
                "   15/5    30/5    60/10                                  i",
                "",
                "",
            ],
        );
    }

    #[test]
    fn a_running_focus_shows_time_left_and_progress() {
        let t0 = t0();
        let mut app = App::new(Plan::presets(), false, 2);
        app.start(1, t0);
        insta(
            &render(&app, 80, t0 + MIN * 4 + Duration::from_secs(2)),
            &[
                "",
                "  ▀▀▀▀▀█  █▀▀▀▀▀     █▀▀▀▀▀  █▀▀▀▀█     ■ FOCUS",
                "       █  █       ▄  █       █    █    ends 13:30",
                "  █▀▀▀▀▀  ▀▀▀▀▀█     ▀▀▀▀▀█  █▀▀▀▀█    / 30 min",
                "  █            █  ▀       █  █    █",
                "  █▄▄▄▄▄  ▄▄▄▄▄█     ▄▄▄▄▄█  █▄▄▄▄█    ● ● 2 today",
                "",
                "     ⠁⠅⢽                                            13%   i",
                "",
                "",
            ],
        );
    }

    #[test]
    fn a_narrow_pane_folds_the_side_column_above_the_clock() {
        let t0 = t0();
        let mut app = App::new(Plan::presets(), false, 0);
        app.start(0, t0);
        // With the key list open, to check hints drop to fit.
        app.handle(Action::ToggleKeys, t0);
        insta(
            &render(&app, 44, t0),
            &[
                "   ■ FOCUS   ends 13:15",
                "    ▀█    █▀▀▀▀▀     █▀▀▀▀█  █▀▀▀▀█",
                "     █    █       ▄  █    █  █    █",
                "     █    ▀▀▀▀▀█     █    █  █    █",
                "     █         █  ▀  █    █  █    █",
                "    ▄█▄   ▄▄▄▄▄█     █▄▄▄▄█  █▄▄▄▄█",
                "",
                "                                   0%   i",
                "",
                "  space pause    s skip   r reset   q quit",
            ],
        );
    }

    /// Compares `actual` with `expected`, printing the actual screen on a
    /// mismatch so a deliberate change can be pasted in.
    fn insta(actual: &[String], expected: &[&str]) {
        assert_eq!(
            actual,
            expected,
            "\nactual screen:\n{}\n",
            actual.join("\n")
        );
    }
}
