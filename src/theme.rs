//! Whether the desktop is dark or light, live. The glass and the window both follow it.

mod scheme;
mod watch;

pub use scheme::current;
pub use watch::Watch;

/// Smoked on a dark desktop, light on a light one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// For a dark desktop.
    Smoked,
    /// For a light desktop.
    Light,
}
