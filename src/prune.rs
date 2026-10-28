//! Prune the raw archive by age (`raw_max_age_days`, 0 = keep forever).
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Delete files older than `max_age_days` under `dir`. Returns how many were removed.
pub fn prune(dir: &Path, max_age_days: u64) -> Result<usize, String> {
    if max_age_days == 0 || !dir.exists() {
        return Ok(0);
    }
    let cutoff = SystemTime::now() - Duration::from_secs(max_age_days * 86_400);
    prune_before(dir, cutoff)
}

fn prune_before(dir: &Path, cutoff: SystemTime) -> Result<usize, String> {
    let mut removed = 0;
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            removed += prune_before(&path, cutoff)?;
        } else if path.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < cutoff) {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_keeps_forever() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a"), "x").unwrap();
        assert_eq!(prune(d.path(), 0).unwrap(), 0);
    }
    #[test]
    fn removes_old_keeps_new() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("s")).unwrap();
        std::fs::write(d.path().join("s/a"), "x").unwrap();
        let future = SystemTime::now() + Duration::from_secs(60);
        assert_eq!(prune_before(d.path(), future).unwrap(), 1);
        std::fs::write(d.path().join("b"), "x").unwrap();
        assert_eq!(prune(d.path(), 1).unwrap(), 0);
    }
}
