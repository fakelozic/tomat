//! The rounded rectangle, for both the paint and the blur behind it.

/// The glass's size, in logical pixels. The width leaves the same room beside
/// `MM:SS` as above and below it, and still fits `120:00`.
pub(super) const WIDTH: i32 = 108;
/// See [`WIDTH`].
pub(super) const HEIGHT: i32 = 42;
/// The corner radius, in logical pixels.
pub(super) const RADIUS: f32 = 12.0;

/// Signed distance from (`x`, `y`) to a rounded box at the origin: negative inside.
pub(super) fn distance(x: f32, y: f32, width: f32, height: f32, radius: f32) -> f32 {
    let across = (x - width / 2.0).abs() - (width / 2.0 - radius);
    let down = (y - height / 2.0).abs() - (height / 2.0 - radius);
    across.max(0.0).hypot(down.max(0.0)) + across.max(down).min(0.0) - radius
}

/// How much of a pixel is inside, from its centre's distance to the edge.
pub(super) fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// The area to blur, as one `(x, y, width)` strip per row. Wayland regions are only
/// rectangles, so the corners are stepped. The border covers the steps.
#[expect(
    clippy::cast_precision_loss,
    reason = "the glass is far smaller than f32's exact range"
)]
pub(super) fn blur_strips() -> Vec<(i32, i32, i32)> {
    let inside = |x: i32, y: i32| {
        distance(
            x as f32 + 0.5,
            y as f32 + 0.5,
            WIDTH as f32,
            HEIGHT as f32,
            RADIUS,
        ) <= 0.0
    };
    (0..HEIGHT)
        .filter_map(|y| {
            let left = (0..WIDTH / 2).find(|&x| inside(x, y))?;
            Some((left, y, WIDTH - 2 * left))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_follows_the_corners() {
        let strips = blur_strips();
        assert_eq!(strips.len(), 42);
        assert_eq!(strips[21], (0, 21, WIDTH), "straight sides");
        assert!(strips[0].0 >= 8, "the corner is cut");
        for (top, bottom) in strips.iter().zip(strips.iter().rev()) {
            assert_eq!((top.0, top.2), (bottom.0, bottom.2), "top mirrors bottom");
        }
    }
}
