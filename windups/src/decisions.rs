//! Decisions made by escalation, kept with provenance (source, step, time).
use rusqlite::{Connection, params};

#[allow(clippy::too_many_arguments)]
pub fn record(
    c: &Connection,
    media_id: i64,
    windup: &str,
    step: &str,
    source: &str,
    decision_json: &str,
    note: &str,
) -> rusqlite::Result<i64> {
    c.execute(
        "INSERT INTO decisions(media_id, windup, step, source, decision, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![media_id, windup, step, source, decision_json, note],
    )?;
    Ok(c.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kept_with_provenance() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        c.execute("INSERT INTO media(kind) VALUES ('movie')", [])
            .unwrap();
        record(
            &c,
            1,
            "jack",
            "match",
            "sammy",
            r#"{"year":"2005"}"#,
            "the US one",
        )
        .unwrap();
        let (s, st, at): (String, String, String) = c
            .query_row("SELECT source, step, decided_at FROM decisions", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(
            (s.as_str(), st.as_str(), at.is_empty()),
            ("sammy", "match", false)
        );
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM unresolved_reasons WHERE reason = 'needs_flags'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
