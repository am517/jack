//! Jack's self-description (PROFILE.toml in the repo is the source of truth).
pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[cfg(test)]
mod tests {
    #[test]
    fn profile_parses_and_names_jack() {
        let v: toml::Value = toml::from_str(super::PROFILE).unwrap();
        assert_eq!(v["name"].as_str(), Some("jack"));
    }
}
