//! Sources that disagree: recorded side by side, never settled by assumption.
use rusqlite::{Connection, params};

/// Records (or refreshes) an open conflict on `field`; `values` are (source, value).
/// Values that are equal ignoring case/spacing are not a conflict. Returns whether one was recorded.
pub fn record(
    c: &Connection,
    media_id: i64,
    field: &str,
    values: &[(String, String)],
) -> rusqlite::Result<bool> {
    let norm = |s: &str| {
        s.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut distinct: Vec<String> = values
        .iter()
        .map(|(_, v)| norm(v))
        .filter(|v| !v.is_empty())
        .collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() < 2 {
        return Ok(false);
    }
    let j = values
        .iter()
        .map(|(s, v)| format!("{{\"source\":{},\"value\":{}}}", json_str(s), json_str(v)))
        .collect::<Vec<_>>()
        .join(",");
    c.execute(
        "INSERT INTO conflicts(media_id, field, values_json) VALUES (?1, ?2, ?3) ON CONFLICT(media_id, field) DO UPDATE SET values_json = excluded.values_json, detected_at = CURRENT_TIMESTAMP WHERE status = 'open'",
        params![media_id, field, format!("[{j}]")],
    )?;
    Ok(true)
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o + "\""
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_real_disagreements() {
        let d = tempfile::tempdir().unwrap();
        let c = crate::db::open(&d.path().join("m.db")).unwrap();
        c.execute("INSERT INTO media(kind) VALUES ('movie')", [])
            .unwrap();
        let v = |a: &[(&str, &str)]| {
            a.iter()
                .map(|(s, v)| (s.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        };
        assert!(
            !record(
                &c,
                1,
                "title",
                &v(&[("embedded", "The Matrix"), ("tmdb", "the  matrix")])
            )
            .unwrap()
        );
        assert!(record(&c, 1, "year", &v(&[("embedded", "1998"), ("tmdb", "1999")])).unwrap());
        assert!(
            record(
                &c,
                1,
                "year",
                &v(&[("embedded", "1998"), ("tmdb", "1999"), ("omdb", "\"1999\"")])
            )
            .unwrap()
        );
        let j: String = c
            .query_row("SELECT values_json FROM conflicts", [], |r| r.get(0))
            .unwrap();
        assert!(j.contains("\\\"1999\\\""));
        assert_eq!(
            c.query_row("SELECT count(*) FROM conflicts", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
