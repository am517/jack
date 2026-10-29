//! YouTube Data API: a trailer link. Needs a key; without one it is silently skipped.
use crate::meta::Meta;
use crate::providers::{Http, Provider, Query, enc, s};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct YoutubeCfg {
    pub api_key: String,
    /// Env var read when api_key is empty.
    pub api_key_env: String,
    pub base_url: String,
    /// Added to the search, e.g. "trailer".
    pub search_suffix: String,
    pub watch_url: String,
}

pub struct Youtube<'a> {
    pub cfg: YoutubeCfg,
    pub http: &'a dyn Http,
}

/// One search per title per run (the free quota is about 100 searches a day).
fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

impl Youtube<'_> {
    fn search(&self, term: &str) -> Result<Option<String>, String> {
        let key = format!("{}|{term}", self.cfg.base_url);
        if let Some(hit) = cache().lock().unwrap().get(&key) {
            return Ok(hit.clone());
        }
        let url = format!(
            "{}/search?part=snippet&type=video&maxResults=1&q={}&key={}",
            self.cfg.base_url,
            enc(term),
            enc(&self.cfg.api_key)
        );
        // never let the key reach an error message
        let r = self
            .http
            .get(&url, &[])
            .map_err(|e| e.replace(&self.cfg.api_key, "****"))?;
        let id = s(&r["items"][0]["id"]["videoId"]);
        cache().lock().unwrap().insert(key, id.clone());
        Ok(id)
    }
}

impl Provider for Youtube<'_> {
    fn name(&self) -> &'static str {
        "youtube"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        if self.cfg.api_key.is_empty() {
            return Ok(None);
        }
        let parts = [
            q.title.as_str(),
            q.year.as_deref().unwrap_or(""),
            &self.cfg.search_suffix,
        ];
        let term = parts
            .iter()
            .filter(|x| !x.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let Some(id) = self.search(&term)? else {
            return Ok(None);
        };
        let mut m = Meta::new();
        m.insert("trailer_url".into(), format!("{}{id}", self.cfg.watch_url));
        m.insert("youtube_trailer_id".into(), id);
        Ok(Some(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use serde_json::json;

    #[test]
    fn trailer_with_key_skipped_without_and_cached_per_title() {
        let h = FakeHttp::new(vec![(
            "/search?",
            json!({"items":[{"id":{"videoId":"abc"}}]}),
        )]);
        let mut c = YoutubeCfg {
            base_url: "http://yt-cache-test".into(),
            search_suffix: "trailer".into(),
            watch_url: "https://www.youtube.com/watch?v=".into(),
            ..Default::default()
        };
        let q = Query {
            title: "The Matrix".into(),
            year: Some("1999".into()),
            ..Default::default()
        };
        assert!(
            Youtube {
                cfg: c.clone(),
                http: &h
            }
            .lookup(&q)
            .unwrap()
            .is_none()
        );
        assert!(h.calls.borrow().is_empty(), "no key: no request");
        c.api_key = "K".into();
        let m = Youtube {
            cfg: c.clone(),
            http: &h,
        }
        .lookup(&q)
        .unwrap()
        .unwrap();
        assert_eq!(m["trailer_url"], "https://www.youtube.com/watch?v=abc");
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|u| u.contains("q=The%20Matrix%201999%20trailer"))
        );
        let m2 = Youtube { cfg: c, http: &h }.lookup(&q).unwrap().unwrap();
        assert_eq!(
            (
                m2["youtube_trailer_id"].as_str(),
                h.calls
                    .borrow()
                    .iter()
                    .filter(|u| u.contains("/search?"))
                    .count()
            ),
            ("abc", 1),
            "second lookup is cached"
        );
    }
}
