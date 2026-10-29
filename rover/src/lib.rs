//! Rover (refining), stub. Rover improves other windups, strictly bound by each windup's
//! profile `purpose` and `success_checks`: he proposes changes, tests them against those
//! checks, and never applies anything unreviewed.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

/// A proposed change to one windup. Proposals are only ever handed over for review.
pub struct Proposal {
    pub windup: String,
    pub change: String,
    /// The success checks this change is meant to improve (must come from the windup's profile).
    pub checks: Vec<String>,
}

/// How a proposal fared against the windup's own success checks.
pub trait Refine {
    fn propose(&self, windup_profile: &str) -> Vec<Proposal>;
    /// Passed and failed checks; nothing is applied here.
    fn test(&self, proposal: &Proposal) -> (Vec<String>, Vec<String>);
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"rover\""));
        assert!(super::PROFILE.contains("success_checks"));
    }
}
