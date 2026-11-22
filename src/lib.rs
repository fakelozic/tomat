//! tomat: three focus/break slots and a glass timer. Two programs share this library:
//! `tomat` (the timer and the glass) and `tomat-window` (the window, only while it is open).

pub mod glass;
pub mod host;
pub mod link;
pub mod log;
pub mod paths;
pub mod plan;
mod private_file;
pub mod theme;
pub mod timer;
