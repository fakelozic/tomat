//! Key bindings: turns terminal key events into app [`Action`]s.
//!
//! The one place keys are defined. The hints in `view.rs` (`menu_screen`,
//! `session_screen`) describe these bindings, so change both together.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::pomodoro::Action;

/// The action bound to `key`, or `None` for unbound keys.
#[must_use]
pub fn action_for(key: KeyEvent) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        // Raw mode delivers Ctrl-C as a key, not a signal.
        return matches!(key.code, KeyCode::Char('c')).then_some(Action::Quit);
    }
    Some(match key.code {
        KeyCode::Char('q') => Action::Quit,
        // The presets sit in a row, but j/k work too for vim hands.
        KeyCode::Char('h' | 'k') | KeyCode::Left | KeyCode::Up => Action::Prev,
        KeyCode::Char('l' | 'j') | KeyCode::Right | KeyCode::Down => Action::Next,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Char(' ' | 'p') => Action::TogglePause,
        KeyCode::Char('s') => Action::Skip,
        KeyCode::Char('r') => Action::Reset,
        KeyCode::Char('m') | KeyCode::Esc => Action::Menu,
        KeyCode::Char('i' | '?') => Action::ToggleKeys,
        KeyCode::Char(c @ '1'..='9') => Action::Pick(usize::from(c as u8 - b'1')),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn number_keys_pick_presets_from_zero() {
        assert_eq!(action_for(key(KeyCode::Char('1'))), Some(Action::Pick(0)));
        assert_eq!(action_for(key(KeyCode::Char('9'))), Some(Action::Pick(8)));
        assert_eq!(action_for(key(KeyCode::Char('0'))), None);
    }

    #[test]
    fn vim_and_arrow_keys_move_the_selection() {
        assert_eq!(action_for(key(KeyCode::Char('h'))), Some(Action::Prev));
        assert_eq!(action_for(key(KeyCode::Right)), Some(Action::Next));
        assert_eq!(action_for(key(KeyCode::Char('j'))), Some(Action::Next));
    }

    #[test]
    fn i_and_question_mark_toggle_the_key_list() {
        assert_eq!(
            action_for(key(KeyCode::Char('i'))),
            Some(Action::ToggleKeys)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('?'))),
            Some(Action::ToggleKeys)
        );
    }

    #[test]
    fn ctrl_c_quits_but_other_ctrl_keys_do_nothing() {
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(action_for(ctrl('c')), Some(Action::Quit));
        assert_eq!(action_for(ctrl('s')), None);
    }
}
