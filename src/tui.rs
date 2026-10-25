//! The terminal side: keys in, frames out.
//!
//! Reads the app state and draws it; never changes it. All colors live in
//! `theme`, the big clock font in `digits`, and the key bindings in `keys`.

mod digits;
mod keys;
mod layout;
mod theme;
mod view;
mod widgets;

pub use keys::action_for;
pub use layout::HEIGHT;
pub use view::draw;
pub use widgets::ANIMATION_STEP;
