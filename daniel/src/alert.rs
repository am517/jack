//! When Daniel alerts Walter. Daniel keeps count (in `windup_state`) of the changes since Walter's
//! last patrol and how much they matter; he calls Walter when a threshold is reached, never for
//! work Walter started himself (the loop guard).
use crate::config::{Config, Importance};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    NewEntry,
    StageChange,
    Update,
}

impl Change {
    pub fn weight(self, w: &Importance) -> u32 {
        match self {
            Change::NewEntry => w.new_entry,
            Change::StageChange => w.stage_change,
            Change::Update => w.update,
        }
    }
}

fn get(c: &Connection, w: &str, k: &str) -> rusqlite::Result<Option<String>> {
    c.query_row(
        "SELECT value FROM windup_state WHERE windup = ?1 AND key = ?2",
        [w, k],
        |r| r.get(0),
    )
    .optional()
}

fn put(c: &Connection, k: &str, v: &str) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO windup_state(windup, key, value) VALUES ('daniel', ?1, ?2)
         ON CONFLICT(windup, key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        [k, v],
    )
    .map(|_| ())
}

fn num(c: &Connection, k: &str) -> rusqlite::Result<u32> {
    Ok(get(c, "daniel", k)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// Counts one change. Changes made before Walter's latest patrol (whoever called him) are
/// already covered, so the count restarts then.
pub fn count(c: &Connection, change: Change, w: &Importance) -> rusqlite::Result<()> {
    let patrolled = get(c, "walter", "last_patrol")?;
    let since = get(c, "daniel", "since")?;
    if since.is_none() || patrolled.as_deref() > since.as_deref() {
        put(c, "changes", "0")?;
        put(c, "importance", "0")?;
        let now: String = c.query_row("SELECT strftime('%Y-%m-%d %H:%M:%f', 'now')", [], |r| {
            r.get(0)
        })?;
        put(c, "since", &now)?;
    }
    put(c, "changes", &(num(c, "changes")? + 1).to_string())?;
    put(
        c,
        "importance",
        &(num(c, "importance")? + change.weight(w)).to_string(),
    )
}

/// Why Walter should be called now, or None. UTC throughout.
pub fn due(c: &Connection, cfg: &Config) -> rusqlite::Result<Option<String>> {
    let w = &cfg.walter;
    let (n, imp) = (num(c, "changes")?, num(c, "importance")?);
    if n == 0 {
        return Ok(None);
    }
    if n >= w.max_changes {
        return Ok(Some(format!("{n} changes")));
    }
    if imp >= w.importance {
        return Ok(Some(format!("importance {imp}")));
    }
    // since Walter's last patrol, or since the first change if he never patrolled
    let from = get(c, "walter", "last_patrol")?.or(get(c, "daniel", "since")?);
    let hours: f64 = c.query_row(
        "SELECT (julianday('now') - julianday(?1)) * 24",
        [from],
        |r| Ok(r.get::<_, Option<f64>>(0)?.unwrap_or(0.0)),
    )?;
    Ok((hours >= w.max_hours).then(|| format!("{hours:.0} h since the last patrol")))
}

/// Walter was called: start counting afresh.
pub fn reset(c: &Connection) -> rusqlite::Result<()> {
    put(c, "changes", "0")?;
    put(c, "importance", "0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(max_changes: u32, importance: u32, max_hours: f64) -> Config {
        let mut c = Config::load(None).unwrap();
        c.walter.max_changes = max_changes;
        c.walter.importance = importance;
        c.walter.max_hours = max_hours;
        c
    }

    #[test]
    fn thresholds_count_importance_and_time() {
        let d = tempfile::tempdir().unwrap();
        let c = windups::db::open(&d.path().join("m.db")).unwrap();
        let w = Config::load(None).unwrap().importance;
        assert_eq!(
            due(&c, &cfg(1, 1, 0.0)).unwrap(),
            None,
            "no changes, no call"
        );
        count(&c, Change::Update, &w).unwrap();
        assert_eq!(
            due(&c, &cfg(5, 10, 24.0)).unwrap(),
            None,
            "one small change"
        );
        assert_eq!(
            due(&c, &cfg(1, 10, 24.0)).unwrap().as_deref(),
            Some("1 changes")
        );
        count(&c, Change::NewEntry, &w).unwrap();
        assert_eq!(
            due(&c, &cfg(5, 4, 24.0)).unwrap().as_deref(),
            Some("importance 4")
        );
        // time: pretend the first change was two days ago
        c.execute(
            "UPDATE windup_state SET value = datetime('now', '-48 hours') WHERE key = 'since'",
            [],
        )
        .unwrap();
        assert!(
            due(&c, &cfg(5, 10, 24.0))
                .unwrap()
                .unwrap()
                .ends_with("h since the last patrol")
        );
        reset(&c).unwrap();
        assert_eq!(due(&c, &cfg(1, 1, 0.0)).unwrap(), None);
        // a patrol by anyone covers what came before it
        count(&c, Change::StageChange, &w).unwrap();
        c.execute("INSERT INTO windup_state(windup, key, value) VALUES ('walter', 'last_patrol', strftime('%Y-%m-%d %H:%M:%f', 'now', '+1 second'))", []).unwrap();
        count(&c, Change::Update, &w).unwrap();
        assert_eq!(num(&c, "changes").unwrap(), 1);
    }
}
