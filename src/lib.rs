//! Mary, the summary windup (stub). Writes a short summary of an entry from its stored texts.
//! Runs before Tom (he embeds her summary); never blocks Larry.
use windups::handoff::Fields;

pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub trait Summarizer {
    fn summarize(&self, entry: &Fields) -> Option<String>;
}

/// Stub: no summary yet.
pub struct NoSummary;

impl Summarizer for NoSummary {
    fn summarize(&self, _: &Fields) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_says_nothing() {
        use super::*;
        assert!(NoSummary.summarize(&Fields::new()).is_none());
        assert!(PROFILE.contains("name = \"mary\""));
    }
}
