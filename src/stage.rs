//! Which lifecycle stage an entry is at after this run. Identification is a process, not a goal:
//! stages and allowed moves live in the database (windups::lifecycle); this only names where Jack
//! leaves an entry.

pub const UNDETERMINED: &str = "undetermined";

/// `unresolved`: no answer could be settled on; `verified`: identified by an id (the only proof);
/// `open`: something is still to look into (a conflict, a needed flag).
pub fn of(unresolved: bool, verified: bool, open: bool) -> &'static str {
    match (unresolved, verified, open) {
        (true, _, _) => "not_identified",
        (false, false, _) => "unverified",
        (false, true, true) => "incomplete",
        (false, true, false) => "complete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_an_id_reaches_complete() {
        assert_eq!(of(true, true, false), "not_identified");
        assert_eq!(
            of(false, false, false),
            "unverified",
            "name and year are hints, not proof"
        );
        assert_eq!(of(false, true, true), "incomplete");
        assert_eq!(of(false, true, false), "complete");
    }
}
