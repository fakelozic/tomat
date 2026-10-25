//! A pausable countdown timer.
//!
//! Pure logic: every method takes the current time as an argument instead of
//! reading the clock itself, so tests can drive time forward exactly.
//!
//! Time is wall-clock time ([`SystemTime`]). Unlike a monotonic `Instant`,
//! it keeps counting while the computer is suspended, so a break that should
//! end at 12:30 does end at 12:30 even if the lid was closed. If the clock is
//! set backwards, that stretch counts as no time rather than negative time.

use std::time::{Duration, SystemTime};

/// A countdown of a fixed length that can be started, paused and resumed.
#[derive(Debug, Clone)]
pub struct Timer {
    duration: Duration,
    /// When the current running stretch began; `None` while not running.
    running_since: Option<SystemTime>,
    /// Time counted in earlier stretches, before the last pause.
    banked: Duration,
    /// Set by the first start, so a pause at the very moment of starting
    /// still counts as started.
    started: bool,
}

impl Timer {
    /// Creates a timer of `duration` that has not started yet.
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            running_since: None,
            banked: Duration::ZERO,
            started: false,
        }
    }

    /// Starts or resumes counting at `now`. Does nothing if already running.
    pub fn start(&mut self, now: SystemTime) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
            self.started = true;
        }
    }

    /// Pauses at `now`, keeping the time counted so far.
    pub fn pause(&mut self, now: SystemTime) {
        if let Some(since) = self.running_since.take() {
            self.banked += stretch(since, now);
        }
    }

    /// Pauses if running, otherwise starts.
    pub fn toggle(&mut self, now: SystemTime) {
        if self.is_running() {
            self.pause(now);
        } else {
            self.start(now);
        }
    }

    /// Whether the timer is counting right now.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    /// Whether the timer has ever been started (running or paused mid-way).
    #[must_use]
    pub fn has_started(&self) -> bool {
        self.started
    }

    /// Time counted so far, capped at the full duration.
    #[must_use]
    pub fn elapsed(&self, now: SystemTime) -> Duration {
        let current = self
            .running_since
            .map_or(Duration::ZERO, |since| stretch(since, now));
        (self.banked + current).min(self.duration)
    }

    /// Time left until the countdown ends.
    #[must_use]
    pub fn remaining(&self, now: SystemTime) -> Duration {
        self.duration.saturating_sub(self.elapsed(now))
    }

    /// Fraction done, from `0.0` to `1.0`. A zero-length timer counts as done.
    #[must_use]
    pub fn progress(&self, now: SystemTime) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        self.elapsed(now).as_secs_f64() / self.duration.as_secs_f64()
    }

    /// Whether the full duration has been counted.
    #[must_use]
    pub fn is_finished(&self, now: SystemTime) -> bool {
        self.elapsed(now) >= self.duration
    }
}

/// Time from `since` to `now`; zero if the clock went backwards in between.
fn stretch(since: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(since).unwrap_or(Duration::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);
    /// A fixed starting point, so tests never read the real clock.
    const T0: SystemTime = SystemTime::UNIX_EPOCH;

    fn at(secs: u64) -> SystemTime {
        T0 + Duration::from_secs(secs)
    }

    #[test]
    fn a_new_timer_has_not_started_and_shows_full_time() {
        let timer = Timer::new(MIN);
        assert!(!timer.has_started());
        assert_eq!(timer.remaining(at(300)), MIN);
    }

    #[test]
    fn counts_down_while_running() {
        let mut timer = Timer::new(MIN);
        timer.start(T0);
        assert_eq!(timer.remaining(at(15)), Duration::from_secs(45));
        assert!((timer.progress(at(15)) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn time_spent_paused_is_not_counted() {
        let mut timer = Timer::new(MIN);
        timer.start(T0);
        timer.pause(at(10));
        // Ten minutes away from the desk while paused.
        timer.start(at(610));
        assert_eq!(timer.elapsed(at(620)), Duration::from_secs(20));
        assert!(timer.has_started());
    }

    #[test]
    fn a_pause_at_the_moment_of_starting_still_counts_as_started() {
        let mut timer = Timer::new(MIN);
        timer.start(T0);
        timer.pause(T0);
        assert!(timer.has_started());
        assert!(!timer.is_running());
    }

    #[test]
    fn toggle_flips_between_running_and_paused() {
        let mut timer = Timer::new(MIN);
        timer.toggle(T0);
        assert!(timer.is_running());
        timer.toggle(at(5));
        assert!(!timer.is_running());
        assert_eq!(timer.elapsed(at(50)), Duration::from_secs(5));
    }

    #[test]
    fn starting_twice_does_not_reset_the_stretch() {
        let mut timer = Timer::new(MIN);
        timer.start(T0);
        timer.start(at(30));
        assert_eq!(timer.elapsed(at(40)), Duration::from_secs(40));
    }

    #[test]
    fn finishes_and_caps_at_the_full_duration() {
        let mut timer = Timer::new(MIN);
        timer.start(T0);
        assert!(!timer.is_finished(at(59)));
        assert!(timer.is_finished(at(60)));
        assert_eq!(timer.remaining(at(180)), Duration::ZERO);
        assert!((timer.progress(at(180)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_clock_set_backwards_counts_as_no_time() {
        let mut timer = Timer::new(MIN);
        timer.start(at(100));
        assert_eq!(timer.elapsed(at(40)), Duration::ZERO);
        timer.pause(at(40));
        assert_eq!(timer.remaining(at(500)), MIN);
    }

    #[test]
    fn a_zero_length_timer_is_already_done() {
        let timer = Timer::new(Duration::ZERO);
        assert!(timer.is_finished(T0));
        assert!((timer.progress(T0) - 1.0).abs() < 1e-9);
    }
}
