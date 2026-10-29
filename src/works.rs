//! Works: one row per work, grouping every version of it (see schema v5).
use rusqlite::{Connection, OptionalExtension, params};

/// Adds or refreshes a work and links the entry to it. Ids are merged, never dropped.
pub fn link(
    c: &Connection,
    media_id: i64,
    key: &str,
    reference: &str,
    ids_json: &str,
) -> rusqlite::Result<()> {
    let old: Option<String> = c
        .query_row("SELECT ids FROM works WHERE key = ?1", [key], |r| r.get(0))
        .optional()?
        .flatten();
    let mut ids: serde_json::Map<String, serde_json::Value> = old
        .and_then(|o| serde_json::from_str(&o).ok())
        .unwrap_or_default();
    if let Ok(serde_json::Value::Object(n)) = serde_json::from_str(ids_json) {
        for (k, v) in n {
            ids.entry(k).or_insert(v);
        }
    }
    let ids = serde_json::Value::Object(ids).to_string();
    c.execute(
        "INSERT INTO works(key, reference, ids) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET ids = excluded.ids, last_seen = CURRENT_TIMESTAMP",
        params![key, reference, ids],
    )?;
    c.execute(
        "UPDATE media SET work_key = ?2 WHERE id = ?1",
        params![media_id, key],
    )?;
    Ok(())
}

/// Entries that are versions of the same work.
pub fn versions(c: &Connection, key: &str) -> rusqlite::Result<Vec<i64>> {
    c.prepare("SELECT id FROM media WHERE work_key = ?1 AND lifecycle IS NOT NULL ORDER BY id")?
        .query_map([key], |r| r.get(0))?
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_versions_and_keeps_ids() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        for t in ["a", "b"] {
            c.execute("INSERT INTO media(kind, title, source, lifecycle) VALUES ('movie', ?1, 'merged', 'unverified')", [t]).unwrap();
        }
        link(
            &c,
            1,
            "wk1:x",
            "The Matrix. (1999). [Film].",
            r#"{"imdb_id":"tt1"}"#,
        )
        .unwrap();
        link(
            &c,
            2,
            "wk1:x",
            "The Matrix. (1999). [Film].",
            r#"{"tmdb_id":"603"}"#,
        )
        .unwrap();
        assert_eq!(versions(&c, "wk1:x").unwrap(), [1, 2]);
        let ids: String = c
            .query_row("SELECT ids FROM works", [], |r| r.get(0))
            .unwrap();
        assert!(ids.contains("tt1") && ids.contains("603"));
    }
}
