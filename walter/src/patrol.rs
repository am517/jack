//! The patrol: every entry that isn't complete gets a scheduled return, decided from what the entry
//! holds (stage, latest reason, last_checked) and Walter's [retry] config. Looked at again since the
//! last schedule → rescheduled (one more attempt, longer back-off). Complete or merged → cleared.
use rusqlite::{Connection, params};

/// Brings `returns` in line with the entries. Returns (scheduled, cleared).
pub fn sweep(c: &Connection, cfg: &crate::config::Retry) -> rusqlite::Result<(usize, usize)> {
    let cleared = c.execute("DELETE FROM returns WHERE media_id IN (SELECT id FROM media WHERE lifecycle IN ('complete', 'merged'))", [])?;
    let todo: Vec<(i64, String, String)> = c
        .prepare(
            "SELECT m.id, m.lifecycle,
                    coalesce((SELECT reason FROM status_history h WHERE h.media_id = m.id ORDER BY h.id DESC LIMIT 1), '')
             FROM media m LEFT JOIN returns r ON r.media_id = m.id
             WHERE m.lifecycle IS NOT NULL AND m.lifecycle NOT IN ('complete', 'merged')
               AND (r.media_id IS NULL OR m.last_checked IS NOT r.checked_at)",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    for (id, stage, reason) in &todo {
        let w = cfg.wait_for.get(stage).map(String::as_str);
        windups::returns::schedule(c, *id, stage, reason, w, &cfg.backoff_hours)?;
    }
    Ok((todo.len(), cleared))
}

/// Files of the given entries that still exist.
pub fn files(items: &[windups::returns::Pending]) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = items
        .iter()
        .filter_map(|p| p.path.as_deref())
        .map(std::path::PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    v.dedup();
    v
}

/// Marks the entries as looked at, so the next sweep reschedules them with a longer back-off even
/// if the windup stored nothing.
pub fn touched(c: &Connection, items: &[windups::returns::Pending]) -> rusqlite::Result<()> {
    for p in items {
        c.execute(
            "UPDATE media SET last_checked = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = ?1",
            params![p.media_id],
        )?;
    }
    Ok(())
}
