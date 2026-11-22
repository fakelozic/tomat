//! The timer: focus, break, focus, break, until it is stopped. Pure: time is passed in.
//!
//! It counts wall-clock time, so a break due at 12:30 ends at 12:30 even if the PC slept.
//! A clock set backwards counts as no time.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::plan::Slot;

/// Which half of a round it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Work time, in orange.
    Focus,
    /// Rest time, in green.
    Break,
}

/// A running or paused timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    slot: Slot,
    /// How far into the round it is. Always less than one whole round.
    done: Duration,
    /// When `done` was last brought up to date.
    seen: SystemTime,
    paused: bool,
}

impl Timer {
    /// Starts a slot's focus at `now`.
    #[must_use]
    pub fn start(slot: Slot, now: SystemTime) -> Self {
        Self { slot, done: Duration::ZERO, seen: now, paused: false }
    }

    fn focus(&self) -> Duration {
        Duration::from_secs(u64::from(self.slot.focus()) * 60)
    }

    fn round(&self) -> Duration {
        self.focus() + Duration::from_secs(u64::from(self.slot.rest()) * 60)
    }

    /// Counts the time up to `now`. True if the phase changed.
    pub fn tick(&mut self, now: SystemTime) -> bool {
        let before = self.phase();
        let passed = now.duration_since(self.seen).unwrap_or_default();
        self.seen = now;
        if self.paused {
            return false;
        }
        let round = self.round();
        let total = self.done.saturating_add(passed);
        // Rounds are whole seconds, so the leftover nanoseconds carry over as they are.
        self.done = Duration::new(total.as_secs() % round.as_secs(), total.subsec_nanos());
        self.phase() != before
    }

    /// Focus or break.
    #[must_use]
    pub fn phase(&self) -> Phase {
        if self.done < self.focus() { Phase::Focus } else { Phase::Break }
    }

    /// Time left in this phase.
    #[must_use]
    pub fn left(&self) -> Duration {
        let end = match self.phase() {
            Phase::Focus => self.focus(),
            Phase::Break => self.round(),
        };
        end.saturating_sub(self.done)
    }

    /// The minutes of the phase after this one.
    #[must_use]
    pub fn next_minutes(&self) -> u16 {
        match self.phase() {
            Phase::Focus => self.slot.rest(),
            Phase::Break => self.slot.focus(),
        }
    }

    /// Whether it is paused.
    #[must_use]
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Pauses or resumes at `now`.
    pub fn set_paused(&mut self, paused: bool, now: SystemTime) {
        self.tick(now);
        self.paused = paused;
    }

    /// How long until what it shows changes: the next second, or `None` while paused.
    #[must_use]
    pub fn next_change(&self) -> Option<Duration> {
        (!self.paused).then(|| match self.left().subsec_nanos() {
            0 => Duration::from_secs(1),
            nanos => Duration::from_nanos(nanos.into()),
        })
    }

    /// One line that brings it back after a crash.
    #[must_use]
    pub fn save(&self) -> String {
        let seen = self.seen.duration_since(UNIX_EPOCH).unwrap_or_default();
        let state = if self.paused { "paused" } else { "running" };
        format!("{} {} {} {state}", self.slot, self.done.as_millis(), seen.as_millis())
    }

    /// Brings a saved timer up to `now`. A running one that was gone for over two rounds is let go.
    #[must_use]
    pub fn restore(line: &str, now: SystemTime) -> Option<Self> {
        let mut words = line.split_ascii_whitespace();
        let slot = Slot::parse(words.next()?)?;
        let done = Duration::from_millis(words.next()?.parse().ok()?);
        let seen = UNIX_EPOCH.checked_add(Duration::from_millis(words.next()?.parse().ok()?))?;
        let paused = match words.next()? {
            "paused" => true,
            "running" => false,
            _ => return None,
        };
        let mut timer = Self { slot, done, seen, paused };
        let round = timer.round();
        let gone = now.duration_since(seen).unwrap_or_default();
        if done >= round || (!paused && gone > round * 2) {
            return None;
        }
        timer.tick(now);
        Some(timer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: Duration = Duration::from_hours(500_000);

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + START + Duration::from_secs(seconds)
    }

    fn timer() -> Timer {
        Timer::start(Slot::new(25, 5).expect("valid"), at(0))
    }

    #[test]
    fn focus_then_break_then_focus_again() {
        let mut timer = timer();
        assert_eq!((timer.phase(), timer.left().as_secs()), (Phase::Focus, 1500));
        assert!(!timer.tick(at(1499)));
        assert!(timer.tick(at(1500)), "the break starts by itself");
        assert_eq!((timer.phase(), timer.left().as_secs(), timer.next_minutes()), (Phase::Break, 300, 25));
        assert!(timer.tick(at(1800)), "and so does the next focus");
        assert_eq!((timer.phase(), timer.left().as_secs(), timer.next_minutes()), (Phase::Focus, 1500, 5));
    }

    #[test]
    fn a_pause_stops_it() {
        let mut timer = timer();
        timer.set_paused(true, at(100));
        assert!(!timer.tick(at(9_000)));
        assert_eq!((timer.left().as_secs(), timer.next_change()), (1400, None));
        timer.set_paused(false, at(9_000));
        timer.tick(at(9_100));
        assert_eq!(timer.left().as_secs(), 1300);
    }

    #[test]
    fn sleep_counts_and_lands_in_the_right_phase() {
        let mut timer = timer();
        timer.tick(at(1800 * 40 + 1600));
        assert_eq!((timer.phase(), timer.left().as_secs()), (Phase::Break, 200));
    }

    #[test]
    fn a_clock_set_backwards_counts_as_no_time() {
        let mut timer = timer();
        timer.tick(at(600));
        timer.tick(at(60));
        assert_eq!(timer.left().as_secs(), 900);
        timer.tick(at(120));
        assert_eq!(timer.left().as_secs(), 840);
    }

    #[test]
    fn a_crash_does_not_lose_it() {
        let mut timer = timer();
        timer.tick(at(100));
        let back = Timer::restore(&timer.save(), at(160)).expect("restores");
        assert_eq!(back.left().as_secs(), 1340);
        timer.set_paused(true, at(100));
        let back = Timer::restore(&timer.save(), at(999_999)).expect("a pause keeps");
        assert_eq!(back.left().as_secs(), 1400);
    }

    #[test]
    fn stale_or_broken_saves_are_let_go() {
        assert_eq!(Timer::restore(&timer().save(), at(3601)), None);
        for line in ["", "25/5", "25/5 x 0 running", "25/5 1800000 0 paused", "0/5 1 1 paused", "25/5 1 1 maybe"] {
            assert_eq!(Timer::restore(line, at(0)), None, "{line}");
        }
    }

    #[test]
    fn it_wakes_on_the_second() {
        let mut timer = timer();
        timer.tick(at(3) + Duration::from_millis(250));
        assert_eq!(timer.next_change(), Some(Duration::from_millis(750)));
    }
}
