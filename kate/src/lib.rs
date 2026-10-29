//! Kate, the image windup (stub). Converts artwork: JPEG for embedding in media files (players
//! read it everywhere), JPEG XL for the archive (smaller, lossless from JPEG).
use std::path::{Path, PathBuf};

pub const PROFILE: &str = include_str!("../PROFILE.toml");

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// For embedding in media files.
    EmbedJpeg,
    /// For the image archive.
    ArchiveJxl,
}

pub trait Converter {
    fn convert(&self, src: &Path, to: Target) -> Result<PathBuf, String>;
}

/// Stub: converts nothing yet.
pub struct NoConverter;

impl Converter for NoConverter {
    fn convert(&self, src: &Path, to: Target) -> Result<PathBuf, String> {
        Err(format!(
            "kate: {:?} of {} not implemented yet",
            to,
            src.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_refuses_plainly() {
        use super::*;
        assert!(
            NoConverter
                .convert(Path::new("a.png"), Target::ArchiveJxl)
                .is_err()
        );
        assert!(PROFILE.contains("name = \"kate\""));
    }
}
