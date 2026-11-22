//! The three slots. Pure.

/// The longest a focus or a break can be, in minutes. It is what three digits hold.
pub const MOST: u16 = 999;

/// One slot: a focus and its break, in whole minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    focus: u16,
    rest: u16,
}

impl Slot {
    /// A slot, if both lengths are from 1 to [`MOST`] minutes.
    #[must_use]
    pub fn new(focus: u16, rest: u16) -> Option<Self> {
        let valid = 1..=MOST;
        (valid.contains(&focus) && valid.contains(&rest)).then_some(Self { focus, rest })
    }

    /// The focus, in minutes.
    #[must_use]
    pub fn focus(self) -> u16 {
        self.focus
    }

    /// The break, in minutes.
    #[must_use]
    pub fn rest(self) -> u16 {
        self.rest
    }

    /// Reads `25/5`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (focus, rest) = text.split_once('/')?;
        Self::new(focus.parse().ok()?, rest.parse().ok()?)
    }
}

impl std::fmt::Display for Slot {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}/{}", self.focus, self.rest)
    }
}

/// All three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slots(pub [Slot; 3]);

impl Default for Slots {
    fn default() -> Self {
        Self([Slot { focus: 15, rest: 5 }, Slot { focus: 30, rest: 5 }, Slot { focus: 60, rest: 10 }])
    }
}

impl Slots {
    /// Reads `15/5 30/5 60/10`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut words = text.split_ascii_whitespace();
        let slots = [Slot::parse(words.next()?)?, Slot::parse(words.next()?)?, Slot::parse(words.next()?)?];
        words.next().is_none().then_some(Self(slots))
    }
}

impl std::fmt::Display for Slots {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c] = self.0;
        write!(out, "{a} {b} {c}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_survive_saving() {
        let slots = Slots([
            Slot::new(1, 999).expect("valid"),
            Slot::new(50, 10).expect("valid"),
            Slot::new(999, 1).expect("valid"),
        ]);
        assert_eq!(Slots::parse(&slots.to_string()), Some(slots));
        assert_eq!(Slots::default().to_string(), "15/5 30/5 60/10");
    }

    #[test]
    fn impossible_slots_are_refused() {
        for text in ["", "0/5 30/5 60/10", "15/5 30/5", "1000/5 30/5 60/10", "15/5 30/5 60/10 1/1", "a/b c/d e/f"] {
            assert_eq!(Slots::parse(text), None, "{text}");
        }
    }
}
