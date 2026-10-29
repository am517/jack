//! Works: one row per work with a permanent local id, grouping every version of it.
//! The reference/key may improve over time; the id never changes. Per-service ids sit beside it.
use rusqlite::{Connection, OptionalExtension, params};

pub struct Work<'a> {
    pub key: &'a str,
    pub reference: &'a str,
    /// Not yet built from the original title, language and release.
    pub provisional: bool,
    /// (source, value), e.g. ("imdb_id", "tt0113277").
    pub ids: &'a [(String, String)],
}

/// Finds the work (by a shared service id, else by key) or creates it, links the entry, and returns
/// the work's permanent id. A provisional reference is replaced by a final one; never the reverse.
/// If the improved key already belongs to another work, both stay and a same_work proposal opens.
pub fn link(c: &Connection, media_id: i64, w: &Work) -> rusqlite::Result<i64> {
    let mut found = None;
    for (s, v) in w.ids {
        found = c
            .query_row(
                "SELECT work_id FROM work_ids WHERE source = ?1 AND value = ?2",
                [s, v],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        if found.is_some() {
            break;
        }
    }
    let by_key: Option<i64> = c
        .query_row("SELECT id FROM works WHERE key = ?1", [w.key], |r| r.get(0))
        .optional()?;
    let id = match (found, by_key) {
        (Some(a), Some(b)) if a != b => {
            crate::proposals::propose(c, "same_work", a, b, 0.0, "works")?;
            a
        }
        (Some(a), _) => {
            let prov: bool =
                c.query_row("SELECT provisional FROM works WHERE id = ?1", [a], |r| {
                    r.get(0)
                })?;
            if prov && !w.provisional && by_key.is_none() {
                c.execute(
                    "UPDATE works SET key = ?2, reference = ?3, provisional = 0 WHERE id = ?1",
                    params![a, w.key, w.reference],
                )?;
            }
            a
        }
        (None, Some(b)) => b,
        (None, None) => {
            c.execute(
                "INSERT INTO works(key, reference, provisional) VALUES (?1, ?2, ?3)",
                params![w.key, w.reference, w.provisional],
            )?;
            c.last_insert_rowid()
        }
    };
    c.execute(
        "UPDATE works SET last_seen = CURRENT_TIMESTAMP WHERE id = ?1",
        [id],
    )?;
    for (s, v) in w.ids {
        c.execute(
            "INSERT OR IGNORE INTO work_ids(work_id, source, value) VALUES (?1, ?2, ?3)",
            params![id, s, v],
        )?;
    }
    c.execute(
        "UPDATE media SET work_id = ?2 WHERE id = ?1",
        params![media_id, id],
    )?;
    Ok(id)
}

/// Entries that are versions of the same work.
pub fn versions(c: &Connection, work_id: i64) -> rusqlite::Result<Vec<i64>> {
    c.prepare("SELECT id FROM media WHERE work_id = ?1 AND lifecycle IS NOT NULL ORDER BY id")?
        .query_map([work_id], |r| r.get(0))?
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permanent_id_survives_a_better_reference() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        for t in ["a", "b", "c"] {
            c.execute(
                "INSERT INTO media(kind, title, source) VALUES ('movie', ?1, 'x')",
                [t],
            )
            .unwrap();
        }
        c.execute_batch("UPDATE media SET lifecycle = 'unverified'")
            .unwrap();
        let imdb = [("imdb_id".to_string(), "tt1".to_string())];
        let w1 = link(
            &c,
            1,
            &Work {
                key: "wk1:en",
                reference: "Spirited Away. (2001). [Film].",
                provisional: true,
                ids: &imdb,
            },
        )
        .unwrap();
        // same work, now known by its original title: same id, better reference
        let w2 = link(
            &c,
            2,
            &Work {
                key: "wk1:ja",
                reference: "千と千尋の神隠し. (2001). [Film].",
                provisional: false,
                ids: &imdb,
            },
        )
        .unwrap();
        assert_eq!(w1, w2);
        let r: String = c
            .query_row("SELECT reference FROM works WHERE id = ?1", [w1], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(r.starts_with("千と千尋"));
        // a copy known only by the final reference joins it too
        assert_eq!(
            link(
                &c,
                3,
                &Work {
                    key: "wk1:ja",
                    reference: "x",
                    provisional: false,
                    ids: &[]
                }
            )
            .unwrap(),
            w1
        );
        assert_eq!(versions(&c, w1).unwrap(), [1, 2, 3]);
    }
}
