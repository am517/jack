//! Is this channel also a show TMDB knows (and via it, TheTVDB and IMDb)? One search, one ids call.
use crate::resolver::Candidate;
use crate::config::Config;
use crate::meta::Meta;
use crate::providers::{Http, enc};
use std::fmt::Write as _;

fn tmdb_get(path: &str, cfg: &Config, http: &dyn Http) -> Result<serde_json::Value, String> {
    let t = &cfg.providers.tmdb;
    let mut url = format!("{}{path}", t.base_url);
    let mut h = vec![];
    if !t.read_token.is_empty() {
        h.push(("Authorization", format!("Bearer {}", t.read_token)));
    } else if !t.api_key.is_empty() {
        url += &format!("&api_key={}", enc(&t.api_key));
    } else {
        return Err("tmdb: no key".into());
    }
    http.get(&url, &h).map_err(|e| e.replace(&t.api_key, "****").replace(&t.read_token, "****"))
}

/// Scored show candidates for a channel name.
pub fn candidates(name: &str, cfg: &Config, http: &dyn Http, out: &mut String) -> Vec<Candidate> {
    if !cfg.online.crossover_providers.iter().any(|p| p == "tmdb") {
        return vec![];
    }
    let r = match tmdb_get(&format!("/search/tv?query={}", enc(name)), cfg, http) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(out, "    warning: crossover: {e}");
            return vec![];
        }
    };
    let list = r["results"].as_array().cloned().unwrap_or_default();
    list.iter()
        .take(cfg.online.crossover_max)
        .filter_map(|x| {
            let show = x["name"].as_str()?.to_string();
            let mut m = Meta::new();
            m.insert("kind".into(), "show".into());
            m.insert("tmdb_series_id".into(), x["id"].as_i64()?.to_string());
            if let Some(d) = x["first_air_date"].as_str().filter(|d| d.len() >= 4) {
                m.insert("year".into(), d[..4].into());
            }
            m.insert("title".into(), show.clone());
            m.insert("show".into(), show);
            Some(("tmdb".to_string(), m))
        })
        .collect()
}

/// Adds the show's other ids (TheTVDB, IMDb).
pub fn details(m: &Meta, cfg: &Config, http: &dyn Http, out: &mut String) -> Option<Meta> {
    let id = m.get("tmdb_series_id")?;
    let mut m = m.clone();
    match tmdb_get(&format!("/tv/{id}/external_ids?"), cfg, http) {
        Ok(r) => {
            if let Some(i) = r["imdb_id"].as_str().filter(|s| !s.is_empty()) {
                m.insert("imdb_id".into(), i.into());
            }
            if let Some(i) = r["tvdb_id"].as_i64() {
                m.insert("thetvdb_series_id".into(), i.to_string());
            }
        }
        Err(e) => {
            let _ = writeln!(out, "    warning: crossover ids: {e}");
        }
    }
    Some(m)
}
