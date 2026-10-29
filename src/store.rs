//! Saving what Jack fetched: into media.db, or (no database) a JSON file beside the media.
use crate::config::suffixed;
use crate::meta::Meta;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct StoreCfg {
    pub enabled: bool,
    pub database: String,
    pub fallback_suffix: String,
    pub busy_timeout_secs: u64,
}

/// rating_<source> meta keys stored in ratings.
const RATINGS: [&str; 5] = ["imdb", "rotten_tomatoes", "metacritic", "tmdb", "tvmaze"];

/// "6.3/10" -> (6.3, 10), "36%" -> (36, 100), "8.4" -> (8.4, 10).
fn rating(v: &str) -> Option<(f64, f64)> {
    let v = v.trim();
    let (val, scale) = match (v.strip_suffix('%'), v.split_once('/')) {
        (Some(p), _) => (p, "100"),
        (_, Some((a, b))) => (a, b),
        _ => (v, "10"),
    };
    Some((val.trim().parse().ok()?, scale.trim().parse().ok()?))
}

/// Meta key -> groups.kind.
const GROUPS: [(&str, &str); 2] = [("franchise", "franchise"), ("series_group", "collection")];
/// Meta key -> art kind.
const ART: [(&str, &str); 4] = [
    ("poster_url", "poster"),
    ("backdrop_url", "backdrop"),
    ("still_url", "still"),
    ("season_poster_url", "season"),
];
/// Meta key -> (source, scope) in external_ids.
const IDS: [(&str, &str, &str); 14] = [
    ("youtube_video_id", "youtube", "video"),
    ("x_video_id", "x", "post"),
    ("wikidata_id", "wikidata", ""),
    ("tvmaze_id", "tvmaze", "show"),
    ("tvmaze_episode_id", "tvmaze", "episode"),
    ("itunes_id", "itunes", ""),
    ("youtube_trailer_id", "youtube", "trailer"),
    ("imdb_id", "imdb", ""),
    ("tmdb_id", "tmdb", ""),
    ("tmdb_series_id", "tmdb", "tv"),
    ("tmdb_episode_id", "tmdb", "episode"),
    ("thetvdb_id", "thetvdb", ""),
    ("thetvdb_series_id", "thetvdb", "series"),
    ("thetvdb_episode_id", "thetvdb", "episode"),
];

const SHOW_IDS: [&str; 4] = ["imdb_id", "tmdb_series_id", "thetvdb_series_id", "tmdb_id"];

/// Channel as an entity, crossover show as a group, "unresolved" as a text.
fn online_extras(tx: &Transaction, id: i64, meta: &Meta) -> rusqlite::Result<()> {
    if let Some(name) = meta.get("channel") {
        let site = meta.get("site").map_or("youtube", |s| s.as_str());
        let by_id = |cid: &String| tx.query_row("SELECT entity_id FROM external_ids WHERE source = ?1 AND scope = 'channel' AND value = ?2 AND entity_id IS NOT NULL", params![site, cid], |r| r.get::<_, i64>(0)).optional();
        let found = match meta.get("channel_id") {
            Some(c) => by_id(c)?,
            None => tx.query_row("SELECT id FROM entities WHERE kind = 'organization' AND name = ?1 AND source = ?2", params![name, site], |r| r.get(0)).optional()?,
        };
        let eid = match found {
            Some(e) => e,
            None => {
                tx.execute("INSERT INTO entities(kind, name, source, fetched_at) VALUES ('organization', ?1, ?2, CURRENT_TIMESTAMP)", params![name, site])?;
                let e = tx.last_insert_rowid();
                if let Some(c) = meta.get("channel_id") {
                    tx.execute("INSERT OR IGNORE INTO external_ids(entity_id, source, scope, value) VALUES (?1, ?2, 'channel', ?3)", params![e, site, c])?;
                }
                e
            }
        };
        tx.execute("INSERT OR IGNORE INTO media_entities(media_id, entity_id, role, source) VALUES (?1, ?2, 'channel', ?3)", params![id, eid, site])?;
    }
    if let Some(show) = meta.get("show") {
        tx.execute("INSERT OR IGNORE INTO groups(name, kind, source) VALUES (?1, 'show', 'crossover')", [show])?;
        tx.execute("INSERT OR IGNORE INTO media_groups(media_id, group_id, source) SELECT ?1, id, 'crossover' FROM groups WHERE name = ?2 AND kind = 'show'", params![id, show])?;
    }
    Ok(())
}

fn db_path(cfg: &StoreCfg) -> Result<PathBuf, String> {
    if cfg.database.is_empty() {
        Ok(windups::Settings::load()?.database_path())
    } else {
        Ok(PathBuf::from(&cfg.database))
    }
}

/// Like a person: write it in the shared book; if the book isn't there, jot it down beside the file.
pub fn save(
    path: &Path,
    meta: &Meta,
    found: &[(String, Meta)],
    cfg: &StoreCfg,
) -> Result<String, String> {
    let db = db_path(cfg).and_then(|p| windups::db::open(&p).map(|c| (p, c)));
    match db {
        Ok((p, mut c)) => {
            c.busy_timeout(std::time::Duration::from_secs(cfg.busy_timeout_secs))
                .map_err(|e| e.to_string())?;
            let id = record(&mut c, path, meta, found).map_err(|e| e.to_string())?;
            Ok(format!("database: media #{id} in {}", p.display()))
        }
        Err(e) => {
            let f = suffixed(path, &cfg.fallback_suffix);
            let cands: Vec<_> = found
                .iter()
                .map(|(s, m)| serde_json::json!({"source": s, "metadata": m}))
                .collect();
            let j = serde_json::json!({"file": path.to_string_lossy(), "metadata": meta, "candidates": cands});
            std::fs::write(
                &f,
                serde_json::to_string_pretty(&j).map_err(|e| e.to_string())? + "\n",
            )
            .map_err(|e| format!("{}: {e}", f.display()))?;
            Ok(format!("no database ({e}); saved {}", f.display()))
        }
    }
}

/// Writes the chosen (merged) row plus every provider answer as a candidate row. Returns the chosen id.
pub fn record(
    c: &mut Connection,
    path: &Path,
    meta: &Meta,
    found: &[(String, Meta)],
) -> rusqlite::Result<i64> {
    let tx = c.transaction()?;
    let existing = find_existing(&tx, meta)?;
    let id = upsert_media(&tx, existing, meta, "merged", "chosen", None)?;
    add_details(&tx, id, meta, "merged", true)?;
    for g in meta
        .get("genre")
        .into_iter()
        .flat_map(|g| g.split(','))
        .map(str::trim)
        .filter(|g| !g.is_empty())
    {
        tx.execute("INSERT OR IGNORE INTO genres(name) VALUES (?1)", [g])?;
        tx.execute("INSERT OR IGNORE INTO media_genres(media_id, genre_id, source) SELECT ?1, id, 'merged' FROM genres WHERE name = ?2", params![id, g])?;
    }
    for src in RATINGS {
        if let Some((value, scale)) = meta.get(&format!("rating_{src}")).and_then(|v| rating(v)) {
            let votes = meta
                .get(&format!("rating_{src}_votes"))
                .and_then(|v| v.parse::<i64>().ok());
            tx.execute(
                "INSERT OR REPLACE INTO ratings(media_id, source, value, scale, votes, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                params![id, src, value, scale, votes],
            )?;
        }
    }
    for (k, kind) in GROUPS {
        if let Some(n) = meta.get(k) {
            tx.execute(
                "INSERT OR IGNORE INTO groups(name, kind, source) VALUES (?1, ?2, 'wikidata')",
                params![n, kind],
            )?;
            tx.execute("INSERT OR IGNORE INTO media_groups(media_id, group_id, source) SELECT ?1, id, 'wikidata' FROM groups WHERE name = ?2 AND kind = ?3", params![id, n, kind])?;
        }
    }
    let online = meta.get("kind").is_some_and(|k| k == crate::online::KIND);
    if online {
        online_extras(&tx, id, meta)?;
    }
    // "unresolved": kept with all candidates, so a later run (or Sammy) can decide
    if let Some(r) = meta.get("resolution") {
        tx.execute("INSERT INTO texts(media_id, field, source, text, chosen, fetched_at) VALUES (?1, 'resolution', 'jack', ?2, 1, CURRENT_TIMESTAMP)", params![id, r])?;
    }
    for (k, src, scope) in IDS {
        // an online video's show ids belong to the show, not to the video
        if online && SHOW_IDS.contains(&k) {
            continue;
        }
        if let Some(v) = meta.get(k) {
            tx.execute("INSERT INTO external_ids(media_id, source, scope, value) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(source, scope, value) DO UPDATE SET media_id = excluded.media_id", params![id, src, scope, v])?;
        }
    }
    for (src, m) in found {
        let cid = upsert_media(&tx, None, m, src, "candidate", Some(id))?;
        add_details(&tx, cid, m, src, false)?;
    }
    tx.execute("INSERT INTO files(path, media_id) VALUES (?1, ?2) ON CONFLICT(path) DO UPDATE SET media_id = excluded.media_id", params![path.to_string_lossy(), id])?;
    tx.commit()?;
    Ok(id)
}

/// A re-run updates the same entry (found by any known id) instead of adding a duplicate.
fn find_existing(tx: &Transaction, meta: &Meta) -> rusqlite::Result<Option<i64>> {
    for (k, src, scope) in IDS {
        if let Some(v) = meta.get(k) {
            let r = tx.query_row("SELECT media_id FROM external_ids WHERE source = ?1 AND scope = ?2 AND value = ?3 AND media_id IS NOT NULL", params![src, scope, v], |r| r.get(0)).optional()?;
            if r.is_some() {
                return Ok(r);
            }
        }
    }
    Ok(None)
}

fn upsert_media(
    tx: &Transaction,
    existing: Option<i64>,
    m: &Meta,
    source: &str,
    status: &str,
    canonical: Option<i64>,
) -> rusqlite::Result<i64> {
    let g = |k: &str| m.get(k).cloned();
    let n = |k: &str| {
        m.get(k)
            .and_then(|v| v.get(..4.min(v.len()))?.parse::<i64>().ok())
    };
    let title = g("episode_title").or(g("title")).or(g("show"));
    let raw = serde_json::to_string(m).unwrap_or_default();
    let vals = params![
        g("kind").unwrap_or_else(|| "movie".into()),
        title,
        g("original_title"),
        n("year"),
        m.get("season").and_then(|v| v.parse::<i64>().ok()),
        m.get("episode").and_then(|v| v.parse::<i64>().ok()),
        g("rated"),
        status,
        canonical,
        source,
        raw
    ];
    if let Some(id) = existing {
        tx.execute("DELETE FROM media WHERE canonical_id = ?1", [id])?;
        for t in ["texts", "art", "media_genres", "media_groups", "ratings", "media_entities"] {
            tx.execute(&format!("DELETE FROM {t} WHERE media_id = ?1"), [id])?;
        }
        tx.execute("UPDATE media SET kind=?1, title=?2, original_title=?3, year=?4, season=?5, episode=?6, rated=?7, status=?8, canonical_id=?9, source=?10, raw_json=?11, fetched_at=CURRENT_TIMESTAMP, updated_at=CURRENT_TIMESTAMP WHERE id = ?12", rusqlite::params_from_iter(vals.iter().copied().chain([&id as &dyn rusqlite::ToSql])))?;
        return Ok(id);
    }
    tx.execute("INSERT INTO media(kind, title, original_title, year, season, episode, rated, status, canonical_id, source, raw_json, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)", vals)?;
    Ok(tx.last_insert_rowid())
}

/// Texts and art for one row; on the chosen row they're marked chosen.
fn add_details(
    tx: &Transaction,
    id: i64,
    m: &Meta,
    source: &str,
    chosen: bool,
) -> rusqlite::Result<()> {
    for (field, key) in [
        ("title", "title"),
        ("title", "episode_title"),
        ("description", "description"),
        ("tagline", "tagline"),
    ] {
        if let Some(t) = m.get(key) {
            tx.execute("INSERT INTO texts(media_id, field, source, text, chosen, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)", params![id, field, source, t, chosen])?;
        }
    }
    for (key, kind) in ART {
        if let Some(u) = m.get(key) {
            tx.execute("INSERT INTO art(media_id, kind, source, url, chosen, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)", params![id, kind, source, u, chosen])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn rating_formats() {
        assert_eq!(rating("6.3/10"), Some((6.3, 10.0)));
        assert_eq!(rating("36%"), Some((36.0, 100.0)));
        assert_eq!(rating("41/100"), Some((41.0, 100.0)));
        assert_eq!(rating("8.4"), Some((8.4, 10.0)));
        assert_eq!(rating("N/A"), None);
    }

    fn count(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn online_video_channel_entity_show_group_and_unresolved() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("m.db")).unwrap();
        let m: Meta = [("kind", "online_video"), ("title", "Ep"), ("channel", "The Why Files"), ("channel_id", "UC1"), ("site", "youtube"), ("youtube_video_id", "abcdefghijk"), ("show", "The Why Files"), ("tmdb_series_id", "7"), ("resolution", "unresolved")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        let f = vec![("tmdb".to_string(), m.clone())];
        record(&mut c, &d.path().join("a.mp4"), &m, &f).unwrap();
        record(&mut c, &d.path().join("b.mp4"), &[("youtube_video_id", "zzzzzzzzzzz")].iter().fold(m.clone(), |mut a, (k, v)| { a.insert(k.to_string(), v.to_string()); a }), &f).unwrap();
        assert_eq!(count(&c, "SELECT count(*) FROM entities WHERE kind='organization'"), 1);
        assert_eq!(count(&c, "SELECT count(*) FROM media_entities WHERE role='channel'"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM groups WHERE kind='show'"), 1);
        assert_eq!(count(&c, "SELECT count(*) FROM external_ids WHERE source='tmdb'"), 0);
        assert_eq!(count(&c, "SELECT count(*) FROM texts WHERE field='resolution'"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM media WHERE status='candidate'"), 2);
    }

    #[test]
    fn records_chosen_candidates_texts_art_genres_ids_and_rerun_updates() {
        let d = tempfile::tempdir().unwrap();
        let mut c = windups::db::open(&d.path().join("media.db")).unwrap();
        let merged = mk(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("description", "Neo"),
            ("poster_url", "http://p"),
            ("genre", "action,sci-fi"),
            ("imdb_id", "tt0133093"),
            ("tmdb_id", "603"),
            ("wikidata_id", "Q83495"),
            ("franchise", "The Matrix"),
            ("rating_imdb", "8.7/10"),
            ("rating_imdb_votes", "2100000"),
            ("rating_rotten_tomatoes", "83%"),
            ("rating_tmdb", "8.2"),
        ]);
        let found = vec![
            (
                "tmdb".to_string(),
                mk(&[("title", "The Matrix"), ("poster_url", "http://p")]),
            ),
            (
                "omdb".to_string(),
                mk(&[("title", "Matrix"), ("year", "1999")]),
            ),
        ];
        let id = record(&mut c, Path::new("/m/a.mp4"), &merged, &found).unwrap();
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE status='candidate'"),
            2
        );
        assert_eq!(
            count(&c, "SELECT count(*) FROM media WHERE raw_json IS NULL"),
            0
        );
        assert_eq!(
            count(&c, "SELECT year FROM media WHERE status='chosen'"),
            1999
        );
        assert_eq!(count(&c, "SELECT count(*) FROM texts WHERE chosen=1"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM art"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM media_genres"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM external_ids"), 3);
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM media_groups JOIN groups ON groups.id = group_id WHERE kind = 'franchise'"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM texts_fts WHERE texts_fts MATCH 'neo'"
            ),
            1
        );
        // re-run: same entry, no duplicates
        assert_eq!(
            record(&mut c, Path::new("/m/a.mp4"), &merged, &found).unwrap(),
            id
        );
        assert_eq!(count(&c, "SELECT count(*) FROM media"), 3);
        assert_eq!(count(&c, "SELECT count(*) FROM art"), 2);
        assert_eq!(count(&c, "SELECT count(*) FROM files"), 1);
        assert_eq!(
            count(
                &c,
                "SELECT count(*) FROM ratings WHERE source = 'rotten_tomatoes' AND value = 83 AND scale = 100"
            ),
            1
        );
        assert_eq!(
            count(
                &c,
                "SELECT votes FROM ratings WHERE source = 'imdb' AND value = 8.7 AND scale = 10"
            ),
            2100000
        );
        assert_eq!(count(&c, "SELECT count(*) FROM ratings"), 3);
        assert_eq!(count(&c, "SELECT count(*) FROM media_groups"), 1);
    }

    #[test]
    fn save_uses_db_else_json_beside_media() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("a.mp4");
        let meta = mk(&[("title", "T")]);
        let found = vec![("tmdb".to_string(), meta.clone())];
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("db/media.db").to_string_lossy().into(),
            fallback_suffix: ".jack.json".into(),
            busy_timeout_secs: 1,
        };
        assert!(
            save(&m, &meta, &found, &cfg)
                .unwrap()
                .starts_with("database: media #1")
        );
        assert!(!d.path().join("a.mp4.jack.json").exists());
        // unopenable database (its folder is a file): JSON beside the media
        std::fs::write(d.path().join("blocker"), "").unwrap();
        let bad = StoreCfg {
            database: d.path().join("blocker/media.db").to_string_lossy().into(),
            ..cfg
        };
        assert!(
            save(&m, &meta, &found, &bad)
                .unwrap()
                .contains("no database")
        );
        let j: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(d.path().join("a.mp4.jack.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            (
                j["metadata"]["title"].as_str(),
                j["candidates"][0]["source"].as_str()
            ),
            (Some("T"), Some("tmdb"))
        );
    }
}
