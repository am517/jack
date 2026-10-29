//! Scheduled returns: windups finish well. An entry that isn't complete is never forgotten: it
//! comes back at `next_retry_at` (backing off with each attempt) or as soon as what it waits for
//! (`waiting_for`) appears, whichever comes first. A complete entry has no return.
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub media_id: i64,
    pub stage: String,
    pub reason: String,
    pub next_retry_at: Option<String>,
    pub waiting_for: Option<String>,
    pub attempts: i64,
    pub path: Option<String>,
}

/// (Re)schedules a return. `backoff_hours[attempts]` (last value repeats) sets the next time;
/// an empty list means resource-only (then `waiting_for` is required).
pub fn schedule(
    c: &Connection,
    media_id: i64,
    stage: &str,
    reason: &str,
    waiting_for: Option<&str>,
    backoff_hours: &[i64],
) -> rusqlite::Result<()> {
    let attempts: i64 = c
        .query_row(
            "SELECT attempts FROM returns WHERE media_id = ?1",
            [media_id],
            |r| r.get(0),
        )
        .optional()?
        .map_or(0, |a: i64| a + 1);
    let hours = backoff_hours
        .get(attempts as usize)
        .or(backoff_hours.last())
        .copied();
    let waiting_for = waiting_for.filter(|w| !w.is_empty());
    // never let it fall through the cracks: no time and nothing to wait for → retry in a day
    let hours = if hours.is_none() && waiting_for.is_none() {
        Some(24)
    } else {
        hours
    };
    c.execute(
        "INSERT INTO returns(media_id, stage, reason, next_retry_at, waiting_for, attempts, updated_at)
         VALUES (?1, ?2, ?3, CASE WHEN ?4 IS NULL THEN NULL ELSE datetime('now', printf('+%d hours', ?4)) END, ?5, ?6, CURRENT_TIMESTAMP)
         ON CONFLICT(media_id) DO UPDATE SET stage = excluded.stage, reason = excluded.reason, next_retry_at = excluded.next_retry_at,
           waiting_for = excluded.waiting_for, attempts = excluded.attempts, updated_at = CURRENT_TIMESTAMP",
        params![media_id, stage, reason, hours, waiting_for, attempts],
    )?;
    Ok(())
}

/// Done: complete entries (and merged-away ones) have nothing to come back for.
pub fn clear(c: &Connection, media_id: i64) -> rusqlite::Result<()> {
    c.execute("DELETE FROM returns WHERE media_id = ?1", [media_id])?;
    Ok(())
}

/// A capability appeared: everything waiting for it is due now. Returns how many woke.
pub fn wake(c: &Connection, capability: &str) -> rusqlite::Result<usize> {
    c.execute("UPDATE returns SET next_retry_at = datetime('now', '-1 second'), updated_at = CURRENT_TIMESTAMP WHERE waiting_for = ?1", [capability])
}

fn list(c: &Connection, where_: &str) -> rusqlite::Result<Vec<Pending>> {
    let sql = format!(
        "SELECT r.media_id, r.stage, r.reason, r.next_retry_at, r.waiting_for, r.attempts,
                (SELECT path FROM files f WHERE f.media_id = r.media_id ORDER BY f.id LIMIT 1)
         FROM returns r {where_} ORDER BY coalesce(r.next_retry_at, '9999'), r.media_id"
    );
    c.prepare(&sql)?
        .query_map([], |r| {
            Ok(Pending {
                media_id: r.get(0)?,
                stage: r.get(1)?,
                reason: r.get(2)?,
                next_retry_at: r.get(3)?,
                waiting_for: r.get(4)?,
                attempts: r.get(5)?,
                path: r.get(6)?,
            })
        })?
        .collect()
}

/// All pending work.
pub fn pending(c: &Connection) -> rusqlite::Result<Vec<Pending>> {
    list(c, "")
}

/// What is due now.
pub fn due(c: &Connection) -> rusqlite::Result<Vec<Pending>> {
    list(c, "WHERE r.next_retry_at <= datetime('now')")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backoff_wake_and_clear() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        for t in ["a", "b"] {
            c.execute(
                "INSERT INTO media(kind, title, source) VALUES ('movie', ?1, 'x')",
                [t],
            )
            .unwrap();
        }
        schedule(&c, 1, "unverified", "name only", None, &[0, 24]).unwrap();
        schedule(&c, 2, "not_identified", "ambiguous", Some("sammy"), &[]).unwrap();
        assert_eq!(pending(&c).unwrap().len(), 2);
        assert_eq!(
            due(&c)
                .unwrap()
                .iter()
                .map(|p| p.media_id)
                .collect::<Vec<_>>(),
            [1]
        );
        // second attempt backs off to a day
        schedule(&c, 1, "unverified", "name only", None, &[0, 24]).unwrap();
        assert!(due(&c).unwrap().is_empty());
        assert_eq!(pending(&c).unwrap()[0].attempts, 1);
        // Sammy appears: what waited for him is due
        assert_eq!(wake(&c, "sammy").unwrap(), 1);
        assert_eq!(due(&c).unwrap()[0].media_id, 2);
        clear(&c, 2).unwrap();
        assert_eq!(pending(&c).unwrap().len(), 1);
        // nothing to wait for and no time: still comes back
        schedule(&c, 2, "incomplete", "", None, &[]).unwrap();
        assert!(
            pending(&c)
                .unwrap()
                .iter()
                .any(|p| p.media_id == 2 && p.next_retry_at.is_some())
        );
    }
}
