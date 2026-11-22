//! Paints the glass into pixels. Pure: the same face always gives the same pixels.

use std::time::Duration;

use anyhow::Result;

use super::palette::{Colour, palette};
use super::shape::{self, HEIGHT, RADIUS, WIDTH};
use super::{Showing, digits};
use crate::theme::Look;
use crate::timer::Phase;

const SQUARE: f32 = 9.0;
const SQUARE_TOP: f32 = 16.5;
const SPACE: f32 = 11.0;
/// Puts the middle of the digits on the middle of the glass.
const BASELINE: f32 = 28.3;
/// Wayland's scales count in 120ths.
pub(super) const SCALE_ONE: u32 = 120;

/// Everything that changes what the glass looks like. An equal face needs no redraw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Face {
    phase: Phase,
    paused: bool,
    clock: String,
    look: Look,
    frosted: bool,
    scale: u32,
}

impl Face {
    pub(super) fn new(showing: Showing, look: Look, frosted: bool, scale: u32) -> Self {
        Self { phase: showing.phase, paused: showing.paused, clock: clock(showing.left), look, frosted, scale }
    }

    pub(super) fn size(&self) -> (usize, usize) {
        size(self.scale)
    }
}

/// The bitmap's size, rounded the way the fractional-scale protocol says.
fn size(scale: u32) -> (usize, usize) {
    let one = |logical: i32| ((logical.unsigned_abs() * scale + SCALE_ONE / 2) / SCALE_ONE) as usize;
    (one(WIDTH), one(HEIGHT))
}

/// `MM:SS`, rounded up, so a fresh timer reads 30:00 and not 29:59.
fn clock(left: Duration) -> String {
    let seconds = left.as_secs().saturating_add(u64::from(left.subsec_nanos() > 0)).min(999 * 60 + 59);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Premultiplied ARGB, as Wayland wants it.
#[expect(clippy::cast_precision_loss, reason = "the bitmap is a few hundred pixels across")]
pub(super) fn paint(face: &Face) -> Result<Vec<u8>> {
    let (width, height) = face.size();
    let scale = face.scale as f32 / SCALE_ONE as f32;
    let colours = palette(face.look, face.phase, face.paused, face.frosted);
    let (wide, high) = (width as f32, height as f32);

    let group = SQUARE + SPACE + digits::width(&face.clock);
    let square_left = ((WIDTH as f32 - group) / 2.0 * scale).round();
    let text_at = ((square_left + (SQUARE + SPACE) * scale).round(), (BASELINE * scale).round());
    let mut text = vec![0.0; width * height];
    digits::draw(&mut text, (width, height), &face.clock, text_at, scale)?;
    let outline = colours.outline.map(|colour| (colour, spread(&text, (width, height), scale)));

    let square_top = (SQUARE_TOP * scale).round();
    let square_side = (SQUARE * scale).round();
    let mut bytes = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let edge = shape::distance(px, py, wide, high, RADIUS * scale);
            let inside = shape::coverage(edge);
            let mut pixel = [0.0; 4];
            over(&mut pixel, colours.body, inside);
            let border = inside - shape::coverage(edge + colours.border_width * scale);
            over(&mut pixel, colours.border, border);
            let square = shape::distance(px - square_left, py - square_top, square_side, square_side, 1.5 * scale);
            over(&mut pixel, colours.accent, shape::coverage(square));
            let at = y * width + x;
            if let Some((colour, halo)) = &outline {
                over(&mut pixel, *colour, halo[at]);
            }
            over(&mut pixel, colours.text, text[at]);
            bytes.extend_from_slice(&argb(pixel).to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Lays a colour over a premultiplied pixel.
fn over(pixel: &mut [f32; 4], colour: Colour, coverage: f32) {
    let alpha = colour.alpha * coverage;
    for (below, above) in pixel.iter_mut().zip(colour.rgb) {
        *below = above * alpha + *below * (1.0 - alpha);
    }
    pixel[3] = alpha + pixel[3] * (1.0 - alpha);
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to a byte")]
fn argb(pixel: [f32; 4]) -> u32 {
    let [r, g, b, a] = pixel.map(|part| (part.clamp(0.0, 1.0) * 255.0).round() as u8);
    u32::from_be_bytes([a, r, g, b])
}

/// The text grown by about a pixel: its outline.
#[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss, reason = "offsets of a pixel or two")]
fn spread(mask: &[f32], (width, height): (usize, usize), radius: f32) -> Vec<f32> {
    let reach = radius.ceil() as isize;
    let mut near = Vec::new();
    for down in -reach..=reach {
        for across in -reach..=reach {
            let weight = (radius + 1.0 - (across as f32).hypot(down as f32)).clamp(0.0, 1.0);
            if weight > 0.0 {
                near.push((across, down, weight));
            }
        }
    }
    let mut grown = vec![0.0_f32; mask.len()];
    for y in 0..height {
        for x in 0..width {
            let covered = mask[y * width + x];
            if covered == 0.0 {
                continue;
            }
            for &(across, down, weight) in &near {
                if let (Some(x), Some(y)) = (x.checked_add_signed(across), y.checked_add_signed(down))
                    && x < width
                    && y < height
                {
                    let pixel = &mut grown[y * width + x];
                    *pixel = pixel.max(covered * weight);
                }
            }
        }
    }
    grown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(look: Look, left: u64, scale: u32) -> Face {
        let showing = Showing { phase: Phase::Focus, paused: false, left: Duration::from_secs(left) };
        Face::new(showing, look, true, scale)
    }

    #[test]
    fn the_clock_rounds_up() {
        assert_eq!(clock(Duration::from_mins(30)), "30:00");
        assert_eq!(clock(Duration::from_millis(1_799_001)), "30:00");
        assert_eq!(clock(Duration::ZERO), "00:00");
        assert_eq!(clock(Duration::from_secs(7200)), "120:00");
        assert_eq!(clock(Duration::MAX), "999:59");
    }

    #[test]
    fn every_pixel_is_premultiplied_and_the_corners_are_clear() {
        for look in [Look::Smoked, Look::Light] {
            for scale in [120, 150, 180, 240, 360] {
                let face = face(look, 1421, scale);
                let (width, height) = face.size();
                let bytes = paint(&face).expect("paints");
                assert_eq!(bytes.len(), width * height * 4);
                for pixel in bytes.as_chunks::<4>().0 {
                    assert!(pixel[..3].iter().all(|&part| part <= pixel[3]));
                }
                assert_eq!(bytes[3], 0, "top left corner");
                assert_eq!(bytes[bytes.len() - 1], 0, "bottom right corner");
            }
        }
    }

    #[test]
    fn the_body_stays_see_through_and_the_time_is_solid() {
        let face = face(Look::Smoked, 1421, 120);
        let bytes = paint(&face).expect("paints");
        let width = face.size().0;
        let alpha = |x: usize, y: usize| bytes[(y * width + x) * 4 + 3];
        assert!((126..=129).contains(&alpha(10, 20)), "50% body");
        assert!((34..94).any(|x| (12..27).any(|y| alpha(x, y) == 255)), "solid digits");
    }

    #[test]
    fn the_square_and_the_time_sit_in_the_middle() {
        for scale in [120, 150, 180, 240] {
            let face = face(Look::Light, 1421, scale);
            let (width, height) = face.size();
            let bytes = paint(&face).expect("paints");
            let (edge_x, edge_y) = (width / 12, height / 8);
            // The square and the digits are solid. The body and border are see-through.
            let solid: Vec<(usize, usize)> = (edge_y..height - edge_y)
                .flat_map(|y| (edge_x..width - edge_x).map(move |x| (x, y)))
                .filter(|&(x, y)| bytes[(y * width + x) * 4 + 3] > 200)
                .collect();
            let left = solid.iter().map(|at| at.0).min().expect("ink");
            let right = solid.iter().map(|at| at.0).max().expect("ink") + 1;
            let top = solid.iter().map(|at| at.1).min().expect("ink");
            let bottom = solid.iter().map(|at| at.1).max().expect("ink") + 1;
            let slack = usize::try_from(scale.div_ceil(120) * 2).expect("small");
            assert!((left + right).abs_diff(width) <= slack * 2, "across: {left}..{right}");
            assert!((top + bottom).abs_diff(height) <= slack, "down: {top}..{bottom}");
        }
    }

    #[test]
    fn the_sides_are_as_roomy_as_the_top_and_long_times_still_fit() {
        let ink = |left: u64| {
            let face = face(Look::Smoked, left, 120);
            let (width, height) = face.size();
            let bytes = paint(&face).expect("paints");
            let solid: Vec<(usize, usize)> = (3..height - 3)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .filter(|&(x, y)| bytes[(y * width + x) * 4 + 3] > 200)
                .collect();
            let left = solid.iter().map(|at| at.0).min().expect("ink");
            let right = width - 1 - solid.iter().map(|at| at.0).max().expect("ink");
            let top = solid.iter().map(|at| at.1).min().expect("ink");
            (left, right, top)
        };
        let (left, right, top) = ink(1421);
        assert!(left.abs_diff(top) <= 3 && right.abs_diff(top) <= 3, "{left} {right} {top}");
        for long in [7200, 999 * 60 + 59] {
            let (left, right, _) = ink(long);
            assert!(left >= 5 && right >= 5, "{long}: {left} {right}");
        }
    }

    #[test]
    fn the_same_second_needs_no_redraw() {
        assert_eq!(face(Look::Light, 1421, 120), face(Look::Light, 1421, 120));
        assert_ne!(face(Look::Light, 1421, 120), face(Look::Light, 1420, 120));
    }
}
