//! The unresolved list: what a windup could not decide, why, and when it was last checked.
use rusqlite::{Connection, params};

/// Opens (or refreshes) an unresolved entry for a media row. One open entry per (media, windup).
pub fn mark(
    c: &Connection,
    media_id: i64,
    windup: &str,
    reason: &str,
    detail: &str,
) -> rusqlite::Result<()> {
    let n = c.execute(
        "UPDATE unresolved SET reason = ?3, detail = ?4, last_checked = CURRENT_TIMESTAMP WHERE media_id = ?1 AND windup = ?2 AND status = 'open'",
        params![media_id, windup, reason, detail],
    )?;
    if n == 0 {
        c.execute(
            "INSERT INTO unresolved(media_id, windup, reason, detail) VALUES (?1, ?2, ?3, ?4)",
            params![media_id, windup, reason, detail],
        )?;
    }
    Ok(())
}

/// Closes open entries once a sure decision exists.
pub fn resolve(c: &Connection, media_id: i64, by: &str) -> rusqlite::Result<usize> {
    c.execute("UPDATE unresolved SET status = 'resolved', resolved_by = ?2, last_checked = CURRENT_TIMESTAMP WHERE media_id = ?1 AND status = 'open'", params![media_id, by])
}

/// Open entries due again: time-based ones last checked at least `retry_days` ago,
/// and resource-based ones whose needed windup is in `available`.
pub fn due(
    c: &Connection,
    retry_days: u64,
    available: &[&str],
) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut st = c.prepare(
        "SELECT u.id, u.reason, r.kind, r.needs_windup, julianday('now') - julianday(u.last_checked) FROM unresolved u JOIN unresolved_reasons r USING (reason) WHERE u.status = 'open' ORDER BY u.id",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, f64>(4)?,
        ))
    })?;
    let mut out = vec![];
    for r in rows {
        let (id, reason, kind, needs, age) = r?;
        let ok = match kind.as_str() {
            "time" => age >= retry_days as f64,
            _ => needs.is_some_and(|n| available.contains(&n.as_str())),
        };
        if ok {
            out.push((id, reason));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mark_due_resolve() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        c.execute("INSERT INTO media(kind) VALUES ('movie'), ('movie')", [])
            .unwrap();
        mark(&c, 1, "jack", "ambiguous", "a / b").unwrap();
        mark(&c, 1, "jack", "ambiguous", "a / b / c").unwrap();
        mark(&c, 2, "jack", "needs_llm_windup", "").unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM unresolved", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(due(&c, 7, &[]).unwrap(), vec![]);
        assert_eq!(due(&c, 0, &[]).unwrap(), vec![(1, "ambiguous".into())]);
        assert_eq!(
            due(&c, 7, &["sammy"]).unwrap(),
            vec![(2, "needs_llm_windup".into())]
        );
        c.execute(
            "UPDATE unresolved SET last_checked = datetime('now', '-8 days') WHERE media_id = 1",
            [],
        )
        .unwrap();
        assert_eq!(due(&c, 7, &[]).unwrap().len(), 1);
        assert_eq!(resolve(&c, 1, "jack").unwrap(), 1);
        assert_eq!(due(&c, 0, &[]).unwrap(), vec![]);
        assert!(mark(&c, 1, "jack", "made_up", "").is_err());
    }
}
