//! The pomodoro itself: plans, the countdown, and the app state machine.
//!
//! Everything here is pure: no terminal, disk, clock or dbus. Time comes in
//! as an argument, and work for the outside world goes out as [`Effect`]s,
//! so all of it is tested without mocks.

mod app;
mod plan;
mod timer;

pub use app::{Action, App, Effect, Session, State, View};
pub use plan::{Phase, Plan, compact, human};
