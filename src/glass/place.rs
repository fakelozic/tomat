//! Where the glass sits: snapping, staying on screen, and remembering it.

use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

use super::shape::{HEIGHT, WIDTH};

/// The gap it keeps from an edge it snapped to, and its gap from the top at first.
pub(super) const GAP: i32 = 12;
/// How close it must come before it snaps.
const SNAP: i32 = 10;

/// A monitor's size, or a point on it, in logical pixels.
pub(super) type Pair = (i32, i32);

/// A saved place, as fractions of the room it has to move in, so it holds when the resolution changes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Spot {
    x: f64,
    y: f64,
    /// The monitor's name, like `DP-1`.
    pub monitor: String,
}

fn room(screen: Pair) -> Pair {
    ((screen.0 - WIDTH).max(0), (screen.1 - HEIGHT).max(0))
}

/// Top centre, where it starts.
pub(super) fn home(screen: Pair) -> Pair {
    let room = room(screen);
    (room.0 / 2, GAP.min(room.1))
}

/// Keeps a point on screen and pulls it gently to the edges and the centre.
pub(super) fn settle(point: Pair, screen: Pair) -> Pair {
    let one = |at: i32, room: i32| {
        let at = at.clamp(0, room);
        [room / 2, GAP, room - GAP]
            .into_iter()
            .find(|&line| (at - line).abs() <= SNAP && (0..=room).contains(&line))
            .unwrap_or(at)
    };
    let room = room(screen);
    (one(point.0, room.0), one(point.1, room.1))
}

impl Spot {
    pub(super) fn new(point: Pair, screen: Pair, monitor: &str) -> Self {
        let part = |at: i32, room: i32| (f64::from(at) / f64::from(room.max(1))).clamp(0.0, 1.0);
        let room = room(screen);
        Self {
            x: part(point.0, room.0),
            y: part(point.1, room.1),
            monitor: monitor.to_owned(),
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "a fraction of an i32 fits in one"
    )]
    pub(super) fn point(&self, screen: Pair) -> Pair {
        let at = |part: f64, room: i32| (part * f64::from(room)).round() as i32;
        let room = room(screen);
        (at(self.x, room.0), at(self.y, room.1))
    }

    fn parse(text: &str) -> Option<Self> {
        let mut words = text.lines().next()?.splitn(3, ' ');
        let mut part = || {
            words
                .next()?
                .parse::<f64>()
                .ok()
                .filter(|n| (0.0..=1.0).contains(n))
        };
        let (x, y) = (part()?, part()?);
        Some(Self {
            x,
            y,
            monitor: words.next().unwrap_or_default().to_owned(),
        })
    }

    /// Reads the saved place. A bad or missing file means top centre.
    pub(super) fn load(path: &Path) -> Option<Self> {
        if !std::fs::metadata(path).ok()?.is_file() {
            return None;
        }
        let mut text = String::new();
        std::fs::File::open(path)
            .ok()?
            .take(256)
            .read_to_string(&mut text)
            .ok()?;
        Self::parse(&text)
    }

    /// Saves the place, readable only by its owner. A crash midway leaves the old one.
    pub(super) fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(folder) = path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(folder)?;
        }
        let draft = path.with_extension("new");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&draft)?;
        writeln!(file, "{} {} {}", self.x, self.y, self.monitor)?;
        file.sync_all()?;
        std::fs::rename(&draft, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Pair = (1920, 1080);

    #[test]
    fn starts_top_centre() {
        assert_eq!(home(SCREEN), (906, 12));
    }

    #[test]
    fn snaps_only_when_close() {
        assert_eq!(settle((913, 525), SCREEN), (906, 519));
        assert_eq!(settle((5, 1030), SCREEN), (12, 1026));
        assert_eq!(settle((300, 300), SCREEN), (300, 300));
    }

    #[test]
    fn stays_on_screen() {
        assert_eq!(settle((-500, 9000), SCREEN), (0, 1038));
        assert_eq!(settle((50, 50), (100, 30)), (0, 0));
    }

    #[test]
    fn a_spot_survives_saving_and_a_new_resolution() {
        let spot = Spot::new(home(SCREEN), SCREEN, "DP-1");
        let read = Spot::parse(&format!("{} {} DP-1\n", spot.x, spot.y)).expect("parses");
        assert_eq!(read, spot);
        assert_eq!(read.point(SCREEN), (906, 12));
        assert_eq!(read.point((2560, 1440)).0, 1226);
    }

    #[test]
    fn a_bad_file_is_ignored() {
        for text in ["", "x y z", "2 0 DP-1", "NaN 0 DP-1", "0.5"] {
            assert_eq!(Spot::parse(text), None, "{text}");
        }
    }
}
