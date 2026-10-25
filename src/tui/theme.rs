//! Colors and styles: gruvbox, to sit well in a gruvbox terminal and tmux.
//!
//! The only place colors are defined; the UI uses these names, never raw
//! values. The terminal's own background shows through everywhere except
//! on chips and the progress bar's track, which sit on [`SURFACE`].

use ratatui::style::{Color, Modifier, Style};

use crate::pomodoro::{Phase, Session, State};

/// Dark text on solid blocks (the selected preset).
pub const INK: Color = Color::Rgb(0x1d, 0x20, 0x21);
/// Raised surfaces: chips and the progress bar's track.
pub const SURFACE: Color = Color::Rgb(0x28, 0x28, 0x28);
/// Main text and the clock digits.
pub const FG: Color = Color::Rgb(0xeb, 0xdb, 0xb2);
/// Quiet text: captions, hint labels, a paused or waiting clock.
pub const GRAY: Color = Color::Rgb(0x92, 0x83, 0x74);
/// Focus, the brand mark and the tally of finished focus sessions.
pub const ORANGE: Color = Color::Rgb(0xfe, 0x80, 0x19);
/// Breaks: a cool mint that reads as "rest" next to the orange.
pub const AQUA: Color = Color::Rgb(0x8e, 0xc0, 0x7c);
/// The dots layered on the progress bar's front: cream sparkles.
pub const SPARK: Color = FG;
/// The paused mark.
pub const YELLOW: Color = Color::Rgb(0xfa, 0xbd, 0x2f);
/// Problems the user should see.
pub const RED: Color = Color::Rgb(0xfb, 0x49, 0x34);

/// The accent color of a phase.
#[must_use]
pub fn phase_color(phase: Phase) -> Color {
    match phase {
        Phase::Focus => ORANGE,
        Phase::Break => AQUA,
    }
}

/// How a session's status chip looks: the mark, its color and the text
/// color. A filled square while running or paused, a hollow one while
/// waiting to start.
#[must_use]
pub fn status_marks(session: &Session) -> (char, Color, Color) {
    let accent = phase_color(session.phase());
    match session.state() {
        State::Running => ('■', accent, FG),
        State::Paused => ('■', YELLOW, FG),
        State::Waiting => ('□', accent, GRAY),
    }
}

/// `color` mixed one part in five into a dark base: the background for a
/// chip whose text is `color`, like a tinted badge.
#[must_use]
pub fn tint(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return SURFACE;
    };
    let mix = |c: u8, base: u8| {
        let mixed = (u16::from(c) + 4 * u16::from(base)) / 5;
        u8::try_from(mixed).unwrap_or(u8::MAX)
    };
    Color::Rgb(mix(r, 0x17), mix(g, 0x19), mix(b, 0x1a))
}

/// A key's name in hints and captions, e.g. `space` in "space to resume".
#[must_use]
pub fn key() -> Style {
    Style::new().fg(FG).add_modifier(Modifier::BOLD)
}

/// A solid block: dark text on `bg`, bold, like tmux's current window.
#[must_use]
pub fn block(bg: Color) -> Style {
    Style::new().fg(INK).bg(bg).add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tint_is_a_dark_shade_of_the_color() {
        assert_eq!(tint(ORANGE), Color::Rgb(0x45, 0x2d, 0x19));
        assert_eq!(tint(Color::Reset), SURFACE);
    }
}
