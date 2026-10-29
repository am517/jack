//! Ben (builds), stub. Ben rebuilds the windups for new platforms (cross-compiles and packages them) so they stay portable with no system services.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_names_me() {
        assert!(super::PROFILE.contains("name = \"ben\""));
    }
}
