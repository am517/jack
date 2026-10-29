//! Perceptual fingerprint: survives re-encode, resize, crop, rotation and aspect changes (sampled
//! video frames, optionally audio). A strong hint only, never proof: a close match opens a
//! "possible_same" proposal for the user or Sammy; entries are never merged automatically.
//! Placeholder. The real one must align the two videos first (by audio, preferred: releases differ
//! in intros, jingles and recaps, so frame N of one is not frame N of the other), then compare
//! frames at matching times (frame pHashes via ffmpeg; Chromaprint for the audio).
//! `NoFingerprint` produces nothing.
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct FingerprintCfg {
    pub enabled: bool,
    /// Frames sampled across the video.
    pub frames: u32,
    /// Also fingerprint the audio.
    pub audio: bool,
    /// Distance (0 = identical, 1 = unrelated) at or below which a proposal is opened.
    pub max_distance: f64,
}

impl Default for FingerprintCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            frames: 16,
            audio: false,
            max_distance: 0.15,
        }
    }
}

pub trait Fingerprinter {
    /// Provenance name for proposals.
    fn name(&self) -> &str;
    fn fingerprint(&self, path: &Path, cfg: &FingerprintCfg) -> Option<String>;
    /// 0 = same content, 1 = unrelated; None = not comparable.
    fn distance(&self, a: &str, b: &str) -> Option<f64>;
}

pub struct NoFingerprint;

impl Fingerprinter for NoFingerprint {
    fn name(&self) -> &str {
        "none"
    }
    fn fingerprint(&self, _: &Path, _: &FingerprintCfg) -> Option<String> {
        None
    }
    fn distance(&self, _: &str, _: &str) -> Option<f64> {
        None
    }
}

/// Opens a possible_same proposal for every other entry whose file is close enough. Returns them.
pub fn propose_similar(
    c: &rusqlite::Connection,
    media_id: i64,
    fp: &str,
    f: &dyn Fingerprinter,
    cfg: &FingerprintCfg,
) -> rusqlite::Result<Vec<i64>> {
    let others: Vec<(i64, String)> = c
        .prepare("SELECT DISTINCT media_id, fingerprint FROM files WHERE fingerprint IS NOT NULL AND media_id IS NOT NULL AND media_id <> ?1")?
        .query_map([media_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut out = vec![];
    for (o, ofp) in others {
        if let Some(d) = f.distance(fp, &ofp).filter(|d| *d <= cfg.max_distance) {
            windups::proposals::propose(c, "possible_same", media_id, o, d, f.name())?;
            out.push(o);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fingerprints as bit strings; distance = share of differing bits.
    struct Bits;
    impl Fingerprinter for Bits {
        fn name(&self) -> &str {
            "bits"
        }
        fn fingerprint(&self, _: &Path, _: &FingerprintCfg) -> Option<String> {
            None
        }
        fn distance(&self, a: &str, b: &str) -> Option<f64> {
            (a.len() == b.len()).then(|| {
                a.chars().zip(b.chars()).filter(|(x, y)| x != y).count() as f64 / a.len() as f64
            })
        }
    }

    #[test]
    fn close_match_is_only_a_proposal() {
        let d = tempfile::tempdir().unwrap();
        let c = windups::db::open(&d.path().join("m.db")).unwrap();
        for (i, fp) in [(1, "0000000000"), (2, "0000000001"), (3, "1111111111")] {
            c.execute("INSERT INTO media(id, kind, title, source, lifecycle) VALUES (?1, 'movie', 't', 'x', 'unverified')", [i]).unwrap();
            c.execute(
                "INSERT INTO files(path, media_id, fingerprint) VALUES (?1, ?2, ?3)",
                rusqlite::params![format!("f{i}"), i, fp],
            )
            .unwrap();
        }
        let cfg = FingerprintCfg::default();
        assert_eq!(
            propose_similar(&c, 1, "0000000000", &Bits, &cfg).unwrap(),
            [2]
        );
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM media WHERE merged_into IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "never merged automatically");
        assert_eq!(windups::proposals::open(&c).unwrap().len(), 1);
        assert!(
            propose_similar(&c, 1, "0000000000", &NoFingerprint, &cfg)
                .unwrap()
                .is_empty()
        );
    }
}
