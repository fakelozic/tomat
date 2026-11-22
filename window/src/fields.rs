//! What is typed in the six fields. Pure.

use tomat::plan::{MOST, Slot, Slots};

/// Only digits, and no more than three.
pub fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).take(3).collect()
}

/// A length that can start.
pub fn valid(text: &str) -> bool {
    text.parse::<u16>().is_ok_and(|minutes| (1..=MOST).contains(&minutes))
}

pub fn slot(focus: &str, rest: &str) -> Option<Slot> {
    Slot::new(focus.parse().ok()?, rest.parse().ok()?)
}

/// Each valid row, and the saved one where a row is half typed.
pub fn merge(saved: Slots, typed: &[[String; 2]; 3]) -> Slots {
    let mut slots = saved;
    for (kept, [focus, rest]) in slots.0.iter_mut().zip(typed) {
        if let Some(slot) = slot(focus, rest) {
            *kept = slot;
        }
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_three_digits_get_in() {
        assert_eq!(digits("2a5"), "25");
        assert_eq!(digits("-12345"), "123");
        assert!(valid("1") && valid("007") && valid("999"));
        assert!(!valid("") && !valid("0") && !valid("1000"));
    }

    #[test]
    fn a_half_typed_row_keeps_what_was_saved() {
        let pair = |focus: &str, rest: &str| [focus.to_owned(), rest.to_owned()];
        let typed = [pair("25", "5"), pair("", "9"), pair("90", "0")];
        assert_eq!(merge(Slots::default(), &typed).to_string(), "25/5 30/5 60/10");
    }
}
