//! The application state machine.
//!
//! Pure logic: it takes [`Action`]s (what the user asked for) and the current
//! time, updates its state, and returns [`Effect`]s (what the outside world
//! should do: notify, record, quit). It never touches the terminal, disk or
//! other processes itself; `main.rs` carries out the effects.

use std::time::{Duration, SystemTime};

use super::plan::{Phase, Plan, human};
use super::timer::Timer;

/// Something the user asked for, already translated from a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Select the previous preset in the menu.
    Prev,
    /// Select the next preset in the menu.
    Next,
    /// Pick the preset at this index and start it.
    Pick(usize),
    /// Start the selected preset, or start a phase that is waiting.
    Confirm,
    /// Pause or resume the running phase.
    TogglePause,
    /// End the current phase early and start the next one.
    Skip,
    /// Restart the current phase from its full length.
    Reset,
    /// Leave the session and go back to the menu.
    Menu,
    /// Show or hide the list of keys.
    ToggleKeys,
    /// Exit the program.
    Quit,
}

/// Work for the outside world, returned by [`App::handle`] and [`App::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Show a desktop notification and ring the terminal bell.
    Notify {
        /// Notification title.
        title: String,
        /// Notification body.
        body: String,
    },
    /// Record a finished focus phase of this length in the history.
    RecordFocus(Duration),
    /// Exit the program.
    Quit,
}

/// Where the current phase's countdown stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Set up, waiting for the user to start it.
    Waiting,
    /// Counting down.
    Running,
    /// Started, then paused.
    Paused,
}

/// A pomodoro in progress: the plan, the current phase and its countdown.
#[derive(Debug, Clone)]
pub struct Session {
    plan: Plan,
    phase: Phase,
    timer: Timer,
    /// Set when the previous phase ran out (rather than being skipped or
    /// reset), so the screen can say so until the next phase starts.
    after_finish: bool,
}

impl Session {
    /// A session of `plan` with its focus phase counting down from `now`.
    fn new(plan: Plan, now: SystemTime) -> Self {
        let mut session = Self {
            plan,
            phase: Phase::Focus,
            timer: Timer::new(plan.focus),
            after_finish: false,
        };
        session.enter(Phase::Focus, now, true);
        session
    }

    /// The plan this session follows.
    #[must_use]
    pub fn plan(&self) -> Plan {
        self.plan
    }

    /// The current phase.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The countdown for the current phase.
    #[must_use]
    pub fn timer(&self) -> &Timer {
        &self.timer
    }

    /// Whether the phase is waiting, running or paused.
    #[must_use]
    pub fn state(&self) -> State {
        if self.timer.is_running() {
            State::Running
        } else if self.timer.has_started() {
            State::Paused
        } else {
            State::Waiting
        }
    }

    /// True while waiting right after the previous phase ran its full length.
    #[must_use]
    pub fn is_after_finish(&self) -> bool {
        self.after_finish && self.state() == State::Waiting
    }

    /// Switches to `phase` with a fresh countdown, running if `run` is set.
    fn enter(&mut self, phase: Phase, now: SystemTime, run: bool) {
        self.phase = phase;
        self.timer = Timer::new(self.plan.length_of(phase));
        if run {
            self.timer.start(now);
        }
    }
}

/// Which screen is showing.
#[derive(Debug, Clone)]
pub enum View {
    /// The preset picker.
    Menu,
    /// A pomodoro in progress.
    Session(Session),
}

/// All state of the program.
#[derive(Debug)]
pub struct App {
    plans: Vec<Plan>,
    selected: usize,
    view: View,
    auto_start: bool,
    today: usize,
    notice: Option<String>,
    show_keys: bool,
}

impl App {
    /// Creates the app on the menu screen.
    ///
    /// * `plans` - the presets to offer; must not be empty.
    /// * `auto_start` - start each next phase without waiting for Enter.
    /// * `today` - focus phases already finished today, from the history.
    #[must_use]
    pub fn new(plans: Vec<Plan>, auto_start: bool, today: usize) -> Self {
        debug_assert!(!plans.is_empty(), "the menu needs at least one plan");
        Self {
            plans,
            selected: 0,
            view: View::Menu,
            auto_start,
            today,
            notice: None,
            show_keys: false,
        }
    }

    /// The presets shown in the menu.
    #[must_use]
    pub fn plans(&self) -> &[Plan] {
        &self.plans
    }

    /// Index of the selected preset; always inside [`App::plans`].
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The screen to draw.
    #[must_use]
    pub fn view(&self) -> &View {
        &self.view
    }

    /// Focus phases finished today.
    #[must_use]
    pub fn today(&self) -> usize {
        self.today
    }

    /// Whether the full list of keys is shown.
    #[must_use]
    pub fn show_keys(&self) -> bool {
        self.show_keys
    }

    /// A one-line problem report to show the user, if any.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Shows a one-line problem report until the next key press.
    pub fn set_notice(&mut self, notice: String) {
        self.notice = Some(notice);
    }

    /// Starts the preset at `index` with a running focus phase.
    pub fn start(&mut self, index: usize, now: SystemTime) {
        let Some(&plan) = self.plans.get(index) else {
            return;
        };
        self.selected = index;
        self.view = View::Session(Session::new(plan, now));
    }

    /// Applies a user action at `now` and returns the effects to carry out.
    pub fn handle(&mut self, action: Action, now: SystemTime) -> Vec<Effect> {
        self.notice = None;
        match action {
            Action::Quit => return vec![Effect::Quit],
            Action::ToggleKeys => {
                self.show_keys = !self.show_keys;
                return Vec::new();
            }
            _ => {}
        }
        match &mut self.view {
            View::Menu => self.handle_menu(action, now),
            View::Session(session) => match action {
                Action::Confirm => session.timer.start(now),
                Action::TogglePause => session.timer.toggle(now),
                // A deliberate skip starts the next phase straight away.
                Action::Skip => {
                    session.after_finish = false;
                    session.enter(session.phase.next(), now, true);
                }
                Action::Reset => {
                    let was_running = session.timer.is_running();
                    session.after_finish = false;
                    session.enter(session.phase, now, was_running);
                }
                Action::Menu => self.view = View::Menu,
                Action::Prev
                | Action::Next
                | Action::Pick(_)
                | Action::ToggleKeys
                | Action::Quit => {}
            },
        }
        Vec::new()
    }

    fn handle_menu(&mut self, action: Action, now: SystemTime) {
        // Clamp rather than wrap, so holding a key stops at the end.
        match action {
            Action::Prev => self.selected = self.selected.saturating_sub(1),
            Action::Next => self.selected = (self.selected + 1).min(self.plans.len() - 1),
            Action::Pick(index) => self.start(index, now),
            Action::Confirm => self.start(self.selected, now),
            _ => {}
        }
    }

    /// How long after `now` the screen next changes, or `None` when nothing
    /// is counting down (menu, paused, waiting to start).
    ///
    /// That is the sooner of two moments: the clock, which shows whole
    /// seconds rounded up, changing its second; and the counted time
    /// reaching the next multiple of `animation_step`, where the progress
    /// bar's animation moves on. Sleeping until then draws each change on
    /// time without polling.
    #[must_use]
    pub fn next_change(&self, now: SystemTime, animation_step: Duration) -> Option<Duration> {
        let View::Session(session) = &self.view else {
            return None;
        };
        if session.state() != State::Running {
            return None;
        }
        let timer = &session.timer;
        let second = until_multiple(timer.remaining(now), Duration::from_secs(1));
        let step = until_next_multiple(timer.elapsed(now), animation_step);
        Some(second.min(step))
    }

    /// Checks the clock at `now`; when a phase has run out, switches to the
    /// next one and returns the notification (and history entry for focus).
    pub fn tick(&mut self, now: SystemTime) -> Vec<Effect> {
        let View::Session(session) = &mut self.view else {
            return Vec::new();
        };
        if !session.timer.has_started() || !session.timer.is_finished(now) {
            return Vec::new();
        }

        let done = session.phase;
        let mut effects = Vec::new();
        if done == Phase::Focus {
            self.today += 1;
            effects.push(Effect::RecordFocus(session.plan.focus));
        }
        effects.push(finished_notice(session.plan, done));
        session.after_finish = true;
        session.enter(done.next(), now, self.auto_start);
        effects
    }
}

/// How far a countdown `left` is above the multiple of `unit` just below it,
/// i.e. how long until it crosses that multiple: a full `unit` when it sits
/// exactly on one.
fn until_multiple(left: Duration, unit: Duration) -> Duration {
    match left.as_nanos() % unit.as_nanos().max(1) {
        0 => unit,
        rest => Duration::from_nanos(u64::try_from(rest).unwrap_or(u64::MAX)),
    }
}

/// How long until a count-up `elapsed` reaches the next multiple of `unit`.
fn until_next_multiple(elapsed: Duration, unit: Duration) -> Duration {
    let unit_nanos = unit.as_nanos().max(1);
    let rest = unit_nanos - elapsed.as_nanos() % unit_nanos;
    Duration::from_nanos(u64::try_from(rest).unwrap_or(u64::MAX))
}

/// The notification for finishing `done`, telling the user what comes next.
fn finished_notice(plan: Plan, done: Phase) -> Effect {
    let next = human(plan.length_of(done.next()));
    let (title, body) = match done {
        Phase::Focus => ("Focus done", format!("Take a {next} break.")),
        Phase::Break => ("Break over", format!("Back to focus for {next}.")),
    };
    Effect::Notify {
        title: title.to_owned(),
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);
    const STEP: Duration = Duration::from_millis(200);
    /// A fixed starting point, so tests never read the real clock.
    const T0: SystemTime = SystemTime::UNIX_EPOCH;

    fn app() -> App {
        App::new(Plan::presets(), false, 0)
    }

    fn session(app: &App) -> &Session {
        match app.view() {
            View::Session(s) => s,
            View::Menu => panic!("expected a session"),
        }
    }

    #[test]
    fn menu_selection_stays_inside_the_list() {
        let mut app = app();
        app.handle(Action::Prev, T0);
        assert_eq!(app.selected(), 0);
        for _ in 0..10 {
            app.handle(Action::Next, T0);
        }
        assert_eq!(app.selected(), app.plans().len() - 1);
    }

    #[test]
    fn confirm_starts_the_selected_preset_with_focus_running() {
        let mut app = app();
        app.handle(Action::Next, T0);
        app.handle(Action::Confirm, T0);
        let s = session(&app);
        assert_eq!(s.plan(), Plan::from_minutes(30, 5));
        assert_eq!(s.phase(), Phase::Focus);
        assert_eq!(s.state(), State::Running);
    }

    #[test]
    fn picking_a_number_out_of_range_does_nothing() {
        let mut app = app();
        app.handle(Action::Pick(99), T0);
        assert!(matches!(app.view(), View::Menu));
    }

    #[test]
    fn finishing_focus_records_it_notifies_and_waits_for_the_break() {
        let mut app = app();
        app.start(0, T0);
        assert_eq!(app.tick(T0 + MIN), []);

        let effects = app.tick(T0 + MIN * 15);
        assert_eq!(effects[0], Effect::RecordFocus(MIN * 15));
        assert!(
            matches!(&effects[1], Effect::Notify { body, .. } if body == "Take a 5 min break.")
        );
        assert_eq!(app.today(), 1);

        let s = session(&app);
        assert_eq!(s.phase(), Phase::Break);
        assert_eq!(s.state(), State::Waiting);
        assert!(s.is_after_finish());
        // Waiting is not finishing: no repeated notifications.
        assert_eq!(app.tick(T0 + MIN * 99), []);
    }

    #[test]
    fn the_cycle_goes_focus_break_focus_break() {
        let mut app = App::new(Plan::presets(), true, 0);
        app.start(0, T0);
        let mut phases = vec![session(&app).phase()];
        let mut now = T0;
        for _ in 0..3 {
            let s = session(&app);
            now += s.plan().length_of(s.phase());
            app.tick(now);
            phases.push(session(&app).phase());
        }
        assert_eq!(
            phases,
            [Phase::Focus, Phase::Break, Phase::Focus, Phase::Break]
        );
        assert_eq!(app.today(), 2);
    }

    #[test]
    fn auto_start_runs_the_next_phase_immediately() {
        let mut app = App::new(Plan::presets(), true, 0);
        app.start(0, T0);
        app.tick(T0 + MIN * 15);
        assert_eq!(session(&app).state(), State::Running);
    }

    #[test]
    fn finishing_a_break_does_not_record_focus() {
        let mut app = app();
        app.start(0, T0);
        app.handle(Action::Skip, T0);
        let effects = app.tick(T0 + MIN * 5);
        assert!(!effects.iter().any(|e| matches!(e, Effect::RecordFocus(_))));
        assert_eq!(app.today(), 0);
        assert_eq!(session(&app).phase(), Phase::Focus);
    }

    #[test]
    fn skip_moves_on_without_counting_and_keeps_running() {
        let mut app = app();
        app.start(0, T0);
        assert_eq!(app.handle(Action::Skip, T0), []);
        let s = session(&app);
        assert_eq!(s.phase(), Phase::Break);
        assert_eq!(s.state(), State::Running);
        assert!(!s.is_after_finish());
        assert_eq!(app.today(), 0);
    }

    #[test]
    fn pause_then_reset_restarts_the_phase_waiting() {
        let mut app = app();
        app.start(0, T0);
        app.handle(Action::TogglePause, T0 + MIN / 2);
        assert_eq!(session(&app).state(), State::Paused);
        app.handle(Action::Reset, T0 + MIN);
        let s = session(&app);
        assert_eq!(s.state(), State::Waiting);
        assert!(!s.is_after_finish());
        assert_eq!(s.timer().remaining(T0), MIN * 15);
    }

    #[test]
    fn quit_works_from_any_screen_and_menu_leaves_the_session() {
        let mut app = app();
        assert_eq!(app.handle(Action::Quit, T0), [Effect::Quit]);
        app.start(0, T0);
        app.handle(Action::Menu, T0);
        assert!(matches!(app.view(), View::Menu));
    }

    #[test]
    fn the_next_frame_is_due_at_the_next_second_or_animation_step() {
        let mut app = app();
        assert_eq!(
            app.next_change(T0, STEP),
            None,
            "nothing counts down in the menu"
        );

        app.start(0, T0);
        // At the start: the next animation step comes before the next second.
        assert_eq!(app.next_change(T0, STEP), Some(STEP));
        // 0.25 s in: the next step is at 0.4 s, the next second at 1 s.
        let later = T0 + Duration::from_millis(250);
        assert_eq!(
            app.next_change(later, STEP),
            Some(Duration::from_millis(150))
        );
        // With slow steps, the clock's next second comes first.
        assert_eq!(
            app.next_change(later, Duration::from_secs(5)),
            Some(Duration::from_millis(750))
        );

        app.handle(Action::TogglePause, later);
        assert_eq!(
            app.next_change(later, STEP),
            None,
            "a paused clock doesn't change"
        );
    }

    #[test]
    fn the_key_list_starts_hidden_and_toggles_anywhere() {
        let mut app = app();
        assert!(!app.show_keys());
        app.handle(Action::ToggleKeys, T0);
        assert!(app.show_keys());
        app.start(0, T0);
        app.handle(Action::ToggleKeys, T0);
        assert!(!app.show_keys());
        assert_eq!(
            session(&app).state(),
            State::Running,
            "toggling keys leaves the timer alone"
        );
    }

    #[test]
    fn a_key_press_clears_the_notice() {
        let mut app = app();
        app.set_notice("disk full".to_owned());
        app.handle(Action::Next, T0);
        assert_eq!(app.notice(), None);
    }
}
