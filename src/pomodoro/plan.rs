//! The pomodoro cycle: focus, break, focus, break, ... and how long each lasts.
//!
//! Pure logic with no I/O. There are no rounds or long breaks on purpose:
//! tomat is a plain pomodoro (see `docs/decisions.md`).

use std::time::Duration;

/// One step of the cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Working time.
    Focus,
    /// Resting time.
    Break,
}

impl Phase {
    /// Upper-case name for the label block.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Focus => "FOCUS",
            Self::Break => "BREAK",
        }
    }

    /// The phase that follows this one.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Focus => Self::Break,
            Self::Break => Self::Focus,
        }
    }
}

/// A pair of lengths: how long to focus and how long to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// Length of a focus phase.
    pub focus: Duration,
    /// Length of a break.
    pub rest: Duration,
}

impl Plan {
    /// Builds a plan from whole minutes.
    #[must_use]
    pub const fn from_minutes(focus: u64, rest: u64) -> Self {
        Self {
            focus: Duration::from_secs(focus * 60),
            rest: Duration::from_secs(rest * 60),
        }
    }

    /// The built-in presets shown in the menu: 15/5, 30/5 and 60/10.
    #[must_use]
    pub fn presets() -> Vec<Self> {
        vec![
            Self::from_minutes(15, 5),
            Self::from_minutes(30, 5),
            Self::from_minutes(60, 10),
        ]
    }

    /// How long `phase` lasts in this plan.
    #[must_use]
    pub fn length_of(&self, phase: Phase) -> Duration {
        match phase {
            Phase::Focus => self.focus,
            Phase::Break => self.rest,
        }
    }
}

/// A length for people: `25 min`, or `90 s` when it isn't whole minutes.
#[must_use]
pub fn human(length: Duration) -> String {
    match in_units(length) {
        (minutes, Unit::Minutes) => format!("{minutes} min"),
        (seconds, Unit::Seconds) => format!("{seconds} s"),
    }
}

/// A length in the fewest characters: `25`, or `90s` when it isn't whole minutes.
#[must_use]
pub fn compact(length: Duration) -> String {
    match in_units(length) {
        (minutes, Unit::Minutes) => minutes.to_string(),
        (seconds, Unit::Seconds) => format!("{seconds}s"),
    }
}

/// The unit a length is shown in.
enum Unit {
    Minutes,
    Seconds,
}

/// `length` as whole minutes when it is a whole number of minutes, otherwise
/// as seconds. Part-seconds round up, as the clock does, so the two agree.
fn in_units(length: Duration) -> (u64, Unit) {
    let seconds = u64::try_from(length.as_nanos().div_ceil(1_000_000_000)).unwrap_or(u64::MAX);
    if seconds.is_multiple_of(60) {
        (seconds / 60, Unit::Minutes)
    } else {
        (seconds, Unit::Seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_and_break_simply_alternate() {
        assert_eq!(Phase::Focus.next(), Phase::Break);
        assert_eq!(Phase::Break.next(), Phase::Focus);
    }

    #[test]
    fn presets_are_15_5_30_5_and_60_10() {
        let minutes: Vec<_> = Plan::presets()
            .iter()
            .map(|p| (p.focus.as_secs() / 60, p.rest.as_secs() / 60))
            .collect();
        assert_eq!(minutes, [(15, 5), (30, 5), (60, 10)]);
    }

    #[test]
    fn plans_report_the_length_of_each_phase() {
        let plan = Plan::from_minutes(30, 5);
        assert_eq!(plan.length_of(Phase::Focus), Duration::from_secs(1800));
        assert_eq!(plan.length_of(Phase::Break), Duration::from_secs(300));
    }

    #[test]
    fn lengths_print_as_minutes_unless_they_have_seconds() {
        assert_eq!(human(Duration::from_secs(1500)), "25 min");
        assert_eq!(human(Duration::from_secs(90)), "90 s");
        assert_eq!(compact(Duration::from_secs(600)), "10");
        assert_eq!(compact(Duration::from_secs(6)), "6s");
    }

    #[test]
    fn part_seconds_round_up_like_the_clock() {
        assert_eq!(human(Duration::from_millis(19_800)), "20 s");
        assert_eq!(compact(Duration::from_millis(59_001)), "1");
    }
}
