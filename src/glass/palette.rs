//! Every colour of the glass, in both looks.

use crate::theme::Look;
use crate::timer::Phase;

/// An sRGB colour with an opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Colour {
    pub rgb: [f32; 3],
    pub alpha: f32,
}

impl Colour {
    const fn new(hex: u32, alpha: f32) -> Self {
        let [_, r, g, b] = hex.to_be_bytes();
        Self { rgb: [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0], alpha }
    }
}

/// One look, ready to paint.
#[derive(Debug, Clone, Copy)]
pub(super) struct Palette {
    pub body: Colour,
    pub text: Colour,
    /// The thin dark line around smoked text, which keeps it readable on a white page.
    pub outline: Option<Colour>,
    pub border: Colour,
    pub border_width: f32,
    pub accent: Colour,
}

const ORANGE: u32 = 0x00fe_8019;
const INK: u32 = 0x0017_191a;
const CREAM: u32 = 0x00eb_dbb2;

/// The colours for a state. Without blur the body is nearly solid, so the time still reads.
pub(super) fn palette(look: Look, phase: Phase, paused: bool, frosted: bool) -> Palette {
    let accent = match (look, paused, phase) {
        (Look::Smoked, true, _) => 0x00fa_bd2f,
        (Look::Light, true, _) => 0x00b5_7614,
        (_, false, Phase::Focus) => ORANGE,
        (Look::Smoked, false, Phase::Break) => 0x008e_c07c,
        (Look::Light, false, Phase::Break) => 0x0042_7b58,
    };
    match look {
        Look::Smoked => Palette {
            body: Colour::new(INK, if frosted { 0.50 } else { 0.88 }),
            text: Colour::new(CREAM, 1.0),
            outline: Some(Colour::new(INK, 1.0)),
            border: Colour::new(CREAM, 0.22),
            border_width: 1.0,
            accent: Colour::new(accent, 1.0),
        },
        Look::Light => Palette {
            body: Colour::new(0x00fb_f1c7, if frosted { 0.45 } else { 0.88 }),
            text: Colour::new(INK, 1.0),
            outline: None,
            border: Colour::new(0x00ff_ffff, 0.24),
            border_width: 1.0,
            accent: Colour::new(accent, 1.0),
        },
    }
}
