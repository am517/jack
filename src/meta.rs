//! Metadata as a flat key/value map. Keys: kind (movie|episode), title, year, date, show,
//! season, episode, episode_title, description, genre, poster_url, poster_file, ids
//! (imdb_id, tmdb_id, thetvdb_id, tmdb_series_id, tmdb_episode_id, thetvdb_series_id, thetvdb_episode_id).
use std::collections::BTreeMap;
use std::path::Path;

pub type Meta = BTreeMap<String, String>;

#[cfg(test)]
pub fn load(p: &Path) -> Result<Meta, String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn save(p: &Path, m: &Meta) -> Result<(), String> {
    let t = serde_json::to_string_pretty(m).map_err(|e| e.to_string())?;
    std::fs::write(p, t + "\n").map_err(|e| format!("{}: {e}", p.display()))
}
