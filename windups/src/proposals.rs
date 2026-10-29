//! Proposals: hints worth a look (e.g. "possibly the same as entry X"). Recorded and left open for
//! the user or Sammy; nothing here changes an entry.
use rusqlite::{Connection, params};

/// Opens a proposal once per (kind, pair); returns true if new.
pub fn propose(
    c: &Connection,
    kind: &str,
    media_id: i64,
    other_id: i64,
    score: f64,
    source: &str,
) -> rusqlite::Result<bool> {
    let (a, b) = (media_id.min(other_id), media_id.max(other_id));
    Ok(c.execute("INSERT OR IGNORE INTO proposals(kind, media_id, other_id, score, source) VALUES (?1, ?2, ?3, ?4, ?5)", params![kind, a, b, score, source])? > 0)
}

pub fn open(c: &Connection) -> rusqlite::Result<Vec<(i64, String, i64, i64)>> {
    c.prepare(
        "SELECT id, kind, media_id, other_id FROM proposals WHERE status = 'open' ORDER BY id",
    )?
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn once_per_pair_and_stays_open() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        for t in ["a", "b"] {
            c.execute(
                "INSERT INTO media(kind, title, source) VALUES ('movie', ?1, 'x')",
                [t],
            )
            .unwrap();
        }
        assert!(propose(&c, "possible_same", 2, 1, 0.1, "fp").unwrap());
        assert!(!propose(&c, "possible_same", 1, 2, 0.1, "fp").unwrap());
        assert_eq!(open(&c).unwrap(), [(1, "possible_same".to_string(), 1, 2)]);
    }
}
