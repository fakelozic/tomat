//! The time's characters, from a tiny bundled font (digits and the colon only).

use ab_glyph::{Font, FontRef, PxScale, point};
use anyhow::{Context, Result};

const FONT: &[u8] = include_bytes!("../../assets/digits.ttf");
/// The em size, in logical pixels.
const SIZE: f32 = 20.0;
/// `ab_glyph` sizes by ascent to descent, which is 1320 of this font's 1000 units per em.
const LINE_PER_EM: f32 = 1.32;
/// Every character is this much of an em wide, so the time never shifts as it counts.
const ADVANCE_PER_EM: f32 = 0.6;

/// How wide `text` is, in logical pixels.
#[expect(clippy::cast_precision_loss, reason = "the time is a handful of characters")]
pub(super) fn width(text: &str) -> f32 {
    text.len() as f32 * SIZE * ADVANCE_PER_EM
}

/// Adds `text` to a coverage mask, starting at `at` (left end of the baseline, device pixels).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "pixel positions inside a tiny bitmap"
)]
pub(super) fn draw(mask: &mut [f32], size: (usize, usize), text: &str, at: (f32, f32), scale: f32) -> Result<()> {
    let font = FontRef::try_from_slice(FONT).context("the bundled font is broken")?;
    let em = SIZE * scale;
    for (index, letter) in text.chars().enumerate() {
        let place = point(at.0 + index as f32 * em * ADVANCE_PER_EM, at.1);
        let glyph = font.glyph_id(letter).with_scale_and_position(PxScale::from(em * LINE_PER_EM), place);
        let Some(outline) = font.outline_glyph(glyph) else {
            continue;
        };
        let corner = outline.px_bounds().min;
        let (left, top) = (corner.x as i64, corner.y as i64);
        outline.draw(|x, y, covered| {
            let (Ok(x), Ok(y)) = (usize::try_from(left + i64::from(x)), usize::try_from(top + i64::from(y))) else {
                return;
            };
            if x < size.0 && y < size.1 {
                let pixel = &mut mask[y * size.0 + x];
                *pixel = pixel.max(covered.clamp(0.0, 1.0));
            }
        });
    }
    Ok(())
}
