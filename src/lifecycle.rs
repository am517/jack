//! Where an entry is on its way to being identified. Stages and allowed moves live in the database
//! (lifecycle_states, lifecycle_transitions); every move is kept in status_history.
use rusqlite::{Connection, OptionalExtension, params};

pub fn current(c: &Connection, media_id: i64) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT lifecycle FROM media WHERE id = ?1",
        [media_id],
        |r| r.get(0),
    )
    .optional()
    .map(Option::flatten)
}

/// Moves an entry to `to` if allowed; records who and why. Ok(false): already there.
/// Err when the move is not allowed or the stage is unknown.
pub fn set(
    c: &Connection,
    media_id: i64,
    to: &str,
    actor: &str,
    reason: &str,
) -> Result<bool, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let from = current(c, media_id).map_err(e)?;
    if from.as_deref() == Some(to) {
        return Ok(false);
    }
    let known: bool = c
        .query_row(
            "SELECT count(*) > 0 FROM lifecycle_states WHERE state = ?1",
            [to],
            |r| r.get(0),
        )
        .map_err(e)?;
    if !known {
        return Err(format!("unknown stage {to}"));
    }
    if to == "merged" {
        return Err("use merge() to merge".into());
    }
    if let Some(f) = &from {
        let ok: bool = c.query_row("SELECT count(*) > 0 FROM lifecycle_transitions WHERE from_state = ?1 AND to_state = ?2", params![f, to], |r| r.get(0)).map_err(e)?;
        if !ok {
            return Err(format!("entry {media_id}: {f} -> {to} is not allowed"));
        }
    }
    c.execute(
        "UPDATE media SET lifecycle = ?2 WHERE id = ?1",
        params![media_id, to],
    )
    .map_err(e)?;
    c.execute("INSERT INTO status_history(media_id, from_state, to_state, actor, reason) VALUES (?1, ?2, ?3, ?4, ?5)", params![media_id, from, to, actor, reason]).map_err(e)?;
    Ok(true)
}

/// Follows merged_into to the entry that stands now.
pub fn resolve_id(c: &Connection, mut id: i64) -> rusqlite::Result<i64> {
    for _ in 0..64 {
        match c
            .query_row("SELECT merged_into FROM media WHERE id = ?1", [id], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .optional()?
            .flatten()
        {
            Some(n) => id = n,
            None => break,
        }
    }
    Ok(id)
}

/// Folds `from` into `into`: files and ids move over, `from` stays (stage merged, merged_into set),
/// and the merge is recorded. Nothing is deleted; ids are never reused.
pub fn merge(
    c: &Connection,
    from: i64,
    into: i64,
    actor: &str,
    reason: &str,
) -> Result<(), String> {
    let e = |e: rusqlite::Error| e.to_string();
    let into = resolve_id(c, into).map_err(e)?;
    if from == into {
        return Err("cannot merge an entry into itself".into());
    }
    if current(c, from).map_err(e)?.as_deref() == Some("merged") {
        return Err(format!("entry {from} is already merged"));
    }
    let prev = current(c, from).map_err(e)?;
    c.execute(
        "UPDATE files SET media_id = ?2 WHERE media_id = ?1",
        params![from, into],
    )
    .map_err(e)?;
    c.execute(
        "UPDATE OR IGNORE external_ids SET media_id = ?2 WHERE media_id = ?1",
        params![from, into],
    )
    .map_err(e)?;
    c.execute(
        "UPDATE media SET canonical_id = ?2 WHERE canonical_id = ?1",
        params![from, into],
    )
    .map_err(e)?;
    c.execute(
        "UPDATE media SET lifecycle = 'merged', merged_into = ?2 WHERE id = ?1",
        params![from, into],
    )
    .map_err(e)?;
    c.execute("INSERT INTO status_history(media_id, from_state, to_state, actor, reason) VALUES (?1, ?2, 'merged', ?3, ?4)", params![from, prev, actor, reason]).map_err(e)?;
    // the merged-away entry has nothing left to come back for
    crate::returns::clear(c, from).map_err(e)?;
    c.execute(
        "INSERT INTO merges(from_id, into_id, actor, reason) VALUES (?1, ?2, ?3, ?4)",
        params![from, into, actor, reason],
    )
    .map_err(e)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(c: &Connection, t: &str) -> i64 {
        c.execute(
            "INSERT INTO media(kind, title, source) VALUES ('unknown', ?1, 'file')",
            [t],
        )
        .unwrap();
        c.last_insert_rowid()
    }

    #[test]
    fn walks_stages_with_history_and_merges_traceably() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        let a = entry(&c, "a.mkv");
        assert!(set(&c, a, "undetermined", "jack", "no lookup").unwrap());
        assert!(!set(&c, a, "undetermined", "jack", "again").unwrap());
        assert!(set(&c, a, "unverified", "jack", "name+year").unwrap());
        assert!(set(&c, a, "nonsense", "jack", "").is_err());
        let b = entry(&c, "The Matrix");
        set(&c, b, "complete", "jack", "imdb id").unwrap();
        c.execute(
            "INSERT INTO files(path, media_id) VALUES ('a.mkv', ?1)",
            [a],
        )
        .unwrap();
        merge(&c, a, b, "user", "same film").unwrap();
        assert_eq!(current(&c, a).unwrap().as_deref(), Some("merged"));
        assert_eq!(resolve_id(&c, a).unwrap(), b);
        let f: i64 = c
            .query_row("SELECT media_id FROM files WHERE path = 'a.mkv'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(f, b);
        assert!(
            set(&c, a, "complete", "jack", "").is_err(),
            "merged is final"
        );
        assert!(merge(&c, a, b, "user", "").is_err());
        let h: Vec<String> = c
            .prepare("SELECT to_state FROM status_history WHERE media_id = ?1 ORDER BY id")
            .unwrap()
            .query_map([a], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(h, ["undetermined", "unverified", "merged"]);
        assert!(
            c.execute("DELETE FROM media WHERE id = ?1", [a]).is_err(),
            "entries are never deleted"
        );
        // a new stage is just a row
        c.execute(
            "INSERT INTO lifecycle_states VALUES ('disputed', 5, 'x')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO lifecycle_transitions VALUES ('complete', 'disputed')",
            [],
        )
        .unwrap();
        assert!(set(&c, b, "disputed", "sammy", "two ids").unwrap());
    }
}
