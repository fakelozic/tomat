//! Effects outside the terminal: desktop notifications and the history file.
//!
//! Thin adapters that `main.rs` calls to carry out the app's effects. They
//! never decide anything; the decisions are in `pomodoro`.

mod history;
mod notify;

pub use history::History;
pub use notify::Notifier;
