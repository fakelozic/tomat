//! What the app and its window say to each other, one line at a time. Pure.

use crate::plan::{Slot, Slots};
use crate::timer::Phase;

/// No honest message is longer.
pub const MOST_LINE: usize = 128;

/// The app to its window: what to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The three slots.
    Slots(Slots),
    /// The running timer.
    Running {
        /// Focus or break.
        phase: Phase,
        /// Whether it is paused.
        paused: bool,
        /// Seconds left, rounded up.
        seconds: u32,
        /// The minutes of the break to come, or `None` in the break: then it stops.
        next: Option<u16>,
    },
}

/// The window to the app: what was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// The slots were edited.
    Save(Slots),
    /// A slot's play was pressed.
    Start(Slot),
    /// Pause.
    Pause,
    /// Resume.
    Resume,
    /// Stop: back to the slots.
    Stop,
}

impl View {
    /// One line, newline included.
    #[must_use]
    pub fn encode(&self) -> String {
        match self {
            Self::Slots(slots) => format!("slots {slots}\n"),
            Self::Running { phase, paused, seconds, next } => {
                let phase = match phase {
                    Phase::Focus => "focus",
                    Phase::Break => "break",
                };
                let state = if *paused { "paused" } else { "running" };
                let next = next.map_or_else(|| "stop".to_owned(), |minutes| minutes.to_string());
                format!("timer {phase} {state} {seconds} {next}\n")
            }
        }
    }

    /// Reads one line without its newline.
    #[must_use]
    pub fn decode(line: &str) -> Option<Self> {
        let (kind, rest) = line.split_once(' ')?;
        match kind {
            "slots" => Slots::parse(rest).map(Self::Slots),
            "timer" => {
                let mut words = rest.split(' ');
                let phase = match words.next()? {
                    "focus" => Phase::Focus,
                    "break" => Phase::Break,
                    _ => return None,
                };
                let paused = match words.next()? {
                    "paused" => true,
                    "running" => false,
                    _ => return None,
                };
                let seconds = words.next()?.parse().ok()?;
                let next = match words.next()? {
                    "stop" => None,
                    minutes => Some(minutes.parse().ok()?),
                };
                Some(Self::Running { phase, paused, seconds, next })
            }
            _ => None,
        }
    }
}

impl Ask {
    /// One line, newline included.
    #[must_use]
    pub fn encode(&self) -> String {
        match self {
            Self::Save(slots) => format!("save {slots}\n"),
            Self::Start(slot) => format!("start {slot}\n"),
            Self::Pause => "pause\n".to_owned(),
            Self::Resume => "resume\n".to_owned(),
            Self::Stop => "stop\n".to_owned(),
        }
    }

    /// Reads one line without its newline.
    #[must_use]
    pub fn decode(line: &str) -> Option<Self> {
        match line.split_once(' ') {
            Some(("save", rest)) => Slots::parse(rest).map(Self::Save),
            Some(("start", rest)) => Slot::parse(rest).map(Self::Start),
            Some(_) => None,
            None => match line {
                "pause" => Some(Self::Pause),
                "resume" => Some(Self::Resume),
                "stop" => Some(Self::Stop),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_survives_the_trip() {
        let slot = Slot::new(999, 999).expect("valid");
        for ask in [Ask::Save(Slots([slot; 3])), Ask::Start(slot), Ask::Pause, Ask::Resume, Ask::Stop] {
            let line = ask.encode();
            assert!(line.len() <= MOST_LINE);
            assert_eq!(Ask::decode(line.trim_end()), Some(ask));
        }
        let focus = View::Running { phase: Phase::Focus, paused: true, seconds: 59_940, next: Some(999) };
        let rest = View::Running { phase: Phase::Break, paused: false, seconds: 1, next: None };
        for view in [View::Slots(Slots::default()), focus, rest] {
            assert_eq!(View::decode(view.encode().trim_end()), Some(view));
        }
        assert_eq!(Ask::decode("start 0/0"), None);
        assert_eq!(Ask::decode("pause now"), None);
    }
}
