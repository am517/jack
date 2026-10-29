//! Julie (verification), stub. Julie verifies entries against evidence (ids, the file's own facts, independent sources) and calls Sammy when judgment is needed. Verifying and thinking are separate.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

/// Outcome of checking one entry against evidence.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    Verified(String),
    Refuted(String),
    /// Evidence alone can't settle it: ask Sammy.
    NeedsJudgment(String),
}

pub trait Verifier {
    fn verify(&self, entry: &std::collections::BTreeMap<String, String>) -> Verdict;
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"julie\""));
    }
}
