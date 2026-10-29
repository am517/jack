//! Tom, the vector windup (stub). Embeds texts (Mary's summary first, else the description) for
//! meaning search; vectors go to the shared `vectors` table via Daniel.
pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub trait Embedder {
    /// Model name, stored with the vector.
    fn model(&self) -> &str;
    fn embed(&self, text: &str) -> Option<Vec<f32>>;
}

/// Stub: no vectors yet.
pub struct NoEmbedder;

impl Embedder for NoEmbedder {
    fn model(&self) -> &str {
        "none"
    }
    fn embed(&self, _: &str) -> Option<Vec<f32>> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_embeds_nothing() {
        use super::*;
        assert!(NoEmbedder.embed("x").is_none());
        assert!(PROFILE.contains("name = \"tom\""));
    }
}
