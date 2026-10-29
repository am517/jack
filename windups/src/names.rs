//! Names: every source's name kept untouched, plus one local display name the user may change.
use rusqlite::{Connection, params};

/// "media" or "entities".
fn table(owner: &str) -> &'static str {
    if owner == "entities" {
        "entities"
    } else {
        "media"
    }
}

/// Keeps a source's name and fills the display name if it has none yet.
pub fn add_source_name(
    c: &Connection,
    owner: &str,
    id: i64,
    source: &str,
    name: &str,
) -> rusqlite::Result<()> {
    let col = if table(owner) == "media" {
        "media_id"
    } else {
        "entity_id"
    };
    c.execute(
        &format!("INSERT OR IGNORE INTO source_names({col}, source, name) VALUES (?1, ?2, ?3)"),
        params![id, source, name],
    )?;
    c.execute(
        &format!("UPDATE {} SET display_name = ?2, display_name_set_by = 'auto:' || ?3 WHERE id = ?1 AND display_name IS NULL", table(owner)),
        params![id, name, source],
    )?;
    Ok(())
}

/// The user's own name for it (e.g. "Joe Rogan"); source names stay as they are.
pub fn set_display_name(c: &Connection, owner: &str, id: i64, name: &str) -> rusqlite::Result<()> {
    c.execute(
        &format!(
            "UPDATE {} SET display_name = ?2, display_name_set_by = 'user' WHERE id = ?1",
            table(owner)
        ),
        params![id, name],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_fill_then_user_override_keeps_sources() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        c.execute(
            "INSERT INTO media(kind, title) VALUES ('show', 'The Joe Rogan Experience')",
            [],
        )
        .unwrap();
        add_source_name(&c, "media", 1, "tmdb", "The Joe Rogan Experience").unwrap();
        add_source_name(&c, "media", 1, "youtube", "PowerfulJRE").unwrap();
        set_display_name(&c, "media", 1, "Joe Rogan").unwrap();
        add_source_name(&c, "media", 1, "youtube", "JRE Clips").unwrap();
        let (dn, by, t): (String, String, String) = c
            .query_row(
                "SELECT display_name, display_name_set_by, title FROM media",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (dn.as_str(), by.as_str(), t.as_str()),
            ("Joe Rogan", "user", "The Joe Rogan Experience")
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM source_names", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }
}
