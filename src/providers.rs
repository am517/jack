//! Metadata providers (TMDB, TheTVDB) behind a common trait; HTTP behind a trait for testing.
use crate::meta::Meta;
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;

pub trait Http {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, String>;
    fn post(&self, url: &str, body: &Value) -> Result<Value, String>;
    fn bytes(&self, url: &str) -> Result<Vec<u8>, String>;
}

pub struct Ureq;

fn uerr(url: &str, e: ureq::Error) -> String {
    let u = url.split('?').next().unwrap_or(url);
    match e {
        ureq::Error::Status(c, r) => format!("{u}: HTTP {c} {}", r.into_string().unwrap_or_default().chars().take(200).collect::<String>()),
        e => format!("{u}: {e}"),
    }
}

impl Http for Ureq {
    fn get(&self, url: &str, headers: &[(&str, String)]) -> Result<Value, String> {
        let mut r = ureq::get(url);
        for (k, v) in headers {
            r = r.set(k, v);
        }
        r.call().map_err(|e| uerr(url, e))?.into_json().map_err(|e| e.to_string())
    }
    fn post(&self, url: &str, body: &Value) -> Result<Value, String> {
        ureq::post(url).send_json(body).map_err(|e| uerr(url, e))?.into_json().map_err(|e| e.to_string())
    }
    fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        std::io::Read::read_to_end(&mut ureq::get(url).call().map_err(|e| uerr(url, e))?.into_reader(), &mut v)
            .map_err(|e| e.to_string())?;
        Ok(v)
    }
}

/// What we know about a file before lookup.
#[derive(Debug, Default, Clone)]
pub struct Query {
    pub title: String,
    pub year: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub imdb: Option<String>,
    pub tmdb: Option<String>,
    pub thetvdb: Option<String>,
}

impl Query {
    pub fn is_tv(&self) -> bool {
        self.season.is_some() || self.thetvdb.is_some() || self.tmdb.as_deref().is_some_and(|t| t.contains("/tv/"))
    }
}

pub trait Provider {
    fn name(&self) -> &'static str;
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String>;
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct TmdbCfg {
    pub read_token: String,
    pub api_key: String,
    pub base_url: String,
    pub image_base: String,
    /// TMDB image size segment, e.g. "w780" or "original" (empty: none).
    pub poster_size: String,
    pub language: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct TvdbCfg {
    /// "full" or "thumbnail" (TheTVDB's "_t" variant).
    pub poster_size: String,
    pub api_key: String,
    pub pin: String,
    pub base_url: String,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct ProvidersCfg {
    pub order: Vec<String>,
    pub tmdb: TmdbCfg,
    pub thetvdb: TvdbCfg,
}

pub fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn s(v: &Value) -> Option<String> {
    match v {
        Value::String(x) if !x.is_empty() => Some(x.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn put(m: &mut Meta, k: &str, v: Option<String>) {
    if let Some(v) = v {
        m.insert(k.into(), v);
    }
}

/// First run of digits after `marker` (or anywhere if marker is empty).
fn digits_after(x: &str, marker: &str) -> Option<String> {
    let i = if marker.is_empty() { 0 } else { x.find(marker)? + marker.len() };
    let d: String = x[i..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (!d.is_empty()).then_some(d)
}

pub fn imdb_id(x: &str) -> Option<String> {
    digits_after(x, "tt").map(|d| format!("tt{d}"))
}

pub struct Tmdb<'a> {
    pub cfg: TmdbCfg,
    pub http: &'a dyn Http,
}

impl Tmdb<'_> {
    fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value, String> {
        let mut url = format!("{}{path}?language={}", self.cfg.base_url, enc(&self.cfg.language));
        let mut h = vec![];
        if !self.cfg.read_token.is_empty() {
            h.push(("Authorization", format!("Bearer {}", self.cfg.read_token)));
        } else if !self.cfg.api_key.is_empty() {
            url += &format!("&api_key={}", enc(&self.cfg.api_key));
        } else {
            return Err("no API key; set [providers.tmdb] read_token or api_key in ~/.config/windups/jack/config.toml (or build with JACK_TMDB_TOKEN)".into());
        }
        for (k, v) in params {
            url += &format!("&{k}={}", enc(v));
        }
        self.http.get(&url, &h)
    }
    fn img(&self, v: &Value) -> Option<String> {
        let sz = if self.cfg.poster_size.is_empty() { String::new() } else { format!("/{}", self.cfg.poster_size) };
        s(v).map(|p| format!("{}{sz}{p}", self.cfg.image_base.trim_end_matches('/')))
    }
}

impl Provider for Tmdb<'_> {
    fn name(&self) -> &'static str {
        "tmdb"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        let mut tv = q.is_tv() || q.tmdb.as_deref().is_some_and(|t| t.contains("/tv/"));
        let mut id = q.tmdb.as_deref().and_then(|t| digits_after(t, if t.contains('/') { "/movie/" } else { "" }).or_else(|| digits_after(t, "/tv/")));
        if id.is_none() {
            if let Some(tt) = q.imdb.as_deref().and_then(imdb_id) {
                let r = self.get(&format!("/find/{tt}"), &[("external_source", "imdb_id".into())])?;
                if let Some(x) = r["tv_results"][0]["id"].as_u64() {
                    (tv, id) = (true, Some(x.to_string()));
                } else {
                    id = r["movie_results"][0]["id"].as_u64().map(|x| x.to_string());
                }
            }
        }
        if id.is_none() && !q.title.is_empty() {
            let mut p = vec![("query", q.title.clone())];
            if let Some(y) = &q.year {
                p.push((if tv { "first_air_date_year" } else { "year" }, y.clone()));
            }
            let r = self.get(if tv { "/search/tv" } else { "/search/movie" }, &p)?;
            id = r["results"][0]["id"].as_u64().map(|x| x.to_string());
        }
        let Some(id) = id else { return Ok(None) };
        let d = self.get(&format!("/{}/{id}", if tv { "tv" } else { "movie" }), &[("append_to_response", "external_ids".into())])?;
        let mut m = Meta::new();
        let date = s(&d[if tv { "first_air_date" } else { "release_date" }]);
        put(&mut m, "year", date.as_ref().map(|x| x.chars().take(4).collect()));
        put(&mut m, "description", s(&d["overview"]));
        put(&mut m, "genre", s(&d["genres"][0]["name"]));
        put(&mut m, "poster_url", self.img(&d["poster_path"]));
        put(&mut m, "tmdb_id", Some(id.clone()));
        put(&mut m, "imdb_id", s(&d["imdb_id"]).or_else(|| s(&d["external_ids"]["imdb_id"])));
        if tv {
            put(&mut m, "tmdb_series_id", Some(id.clone()));
            m.insert("kind".into(), "episode".into());
            put(&mut m, "show", s(&d["name"]));
            put(&mut m, "title", s(&d["name"]));
            if let Some(se) = q.season {
                // season poster is optional; ignore failures
                if let Ok(sd) = self.get(&format!("/tv/{id}/season/{se}"), &[]) {
                    put(&mut m, "season_poster_url", self.img(&sd["poster_path"]));
                }
            }
            if let (Some(se), Some(ep)) = (q.season, q.episode) {
                let e = self.get(&format!("/tv/{id}/season/{se}/episode/{ep}"), &[])?;
                m.insert("season".into(), se.to_string());
                m.insert("episode".into(), ep.to_string());
                put(&mut m, "episode_title", s(&e["name"]));
                put(&mut m, "description", s(&e["overview"]));
                put(&mut m, "date", s(&e["air_date"]));
                put(&mut m, "tmdb_episode_id", s(&e["id"]));
            }
        } else {
            m.insert("kind".into(), "movie".into());
            put(&mut m, "title", s(&d["title"]));
            put(&mut m, "date", date);
        }
        Ok(Some(m))
    }
}

pub struct Tvdb<'a> {
    pub cfg: TvdbCfg,
    pub http: &'a dyn Http,
    token: RefCell<Option<String>>,
}

impl<'a> Tvdb<'a> {
    pub fn new(cfg: TvdbCfg, http: &'a dyn Http) -> Self {
        Tvdb { cfg, http, token: RefCell::new(None) }
    }
    fn get(&self, path: &str) -> Result<Value, String> {
        if self.token.borrow().is_none() {
            if self.cfg.api_key.is_empty() {
                return Err("no API key; set [providers.thetvdb] api_key (and pin if required) in ~/.config/windups/jack/config.toml (or build with JACK_TVDB_KEY)".into());
            }
            let mut b = json!({ "apikey": self.cfg.api_key });
            if !self.cfg.pin.is_empty() {
                b["pin"] = json!(self.cfg.pin);
            }
            let r = self.http.post(&format!("{}/login", self.cfg.base_url), &b)?;
            let t = s(&r["data"]["token"]).ok_or("TheTVDB: login returned no token")?;
            *self.token.borrow_mut() = Some(t);
        }
        let t = self.token.borrow().clone().unwrap_or_default();
        Ok(self.http.get(&format!("{}{path}", self.cfg.base_url), &[("Authorization", format!("Bearer {t}"))])?["data"].clone())
    }
}

impl Provider for Tvdb<'_> {
    fn name(&self) -> &'static str {
        "thetvdb"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        if !q.is_tv() {
            return Ok(None);
        }
        let series = match q.thetvdb.as_deref() {
            Some(x) if x.chars().all(|c| c.is_ascii_digit()) => self.get(&format!("/series/{x}"))?,
            Some(x) if x.contains("/series/") => {
                let slug = x.split("/series/").nth(1).unwrap_or("").split(['/', '?', '#']).next().unwrap_or("");
                self.get(&format!("/series/slug/{}", enc(slug)))?
            }
            _ if !q.title.is_empty() => {
                let yr = q.year.as_ref().map(|y| format!("&year={}", enc(y))).unwrap_or_default();
                let r = self.get(&format!("/search?type=series&query={}{yr}", enc(&q.title)))?;
                let Some(id) = s(&r[0]["tvdb_id"]) else { return Ok(None) };
                self.get(&format!("/series/{id}"))?
            }
            _ => return Ok(None),
        };
        let Some(id) = s(&series["id"]) else { return Ok(None) };
        let mut m = Meta::new();
        m.insert("kind".into(), "episode".into());
        put(&mut m, "show", s(&series["name"]));
        put(&mut m, "title", s(&series["name"]));
        put(&mut m, "year", s(&series["year"]));
        put(&mut m, "description", s(&series["overview"]));
        let img = |u: Option<String>| match self.cfg.poster_size.as_str() {
            "thumbnail" => u.map(|u| match u.rfind('.') {
                Some(i) if !u[i..].contains('/') => format!("{}_t{}", &u[..i], &u[i..]),
                _ => u,
            }),
            _ => u,
        };
        put(&mut m, "poster_url", img(s(&series["image"])));
        if let Some(se) = q.season {
            if let Ok(x) = self.get(&format!("/series/{}/extended?short=true", s(&series["id"]).unwrap_or_default())) {
                let ss = x["seasons"].as_array().cloned().unwrap_or_default();
                let hit = ss.iter().find(|v| v["number"].as_u64() == Some(se as u64) && v["type"]["type"].as_str().is_none_or(|t| t == "official"));
                put(&mut m, "season_poster_url", img(hit.and_then(|v| s(&v["image"]))));
            }
        }
        put(&mut m, "thetvdb_id", Some(id.clone()));
        put(&mut m, "thetvdb_series_id", Some(id.clone()));
        if let (Some(se), Some(ep)) = (q.season, q.episode) {
            let r = self.get(&format!("/series/{id}/episodes/default?page=0&season={se}&episodeNumber={ep}"))?;
            let e = &r["episodes"][0];
            m.insert("season".into(), se.to_string());
            m.insert("episode".into(), ep.to_string());
            put(&mut m, "episode_title", s(&e["name"]));
            put(&mut m, "description", s(&e["overview"]));
            put(&mut m, "date", s(&e["aired"]));
            put(&mut m, "thetvdb_episode_id", s(&e["id"]));
        }
        Ok(Some(m))
    }
}

#[cfg(test)]
pub mod fake {
    use super::*;
    /// Responds with the first route whose key is a substring of the URL; records calls.
    pub struct FakeHttp {
        pub routes: Vec<(&'static str, Value)>,
        pub calls: RefCell<Vec<String>>,
    }
    impl FakeHttp {
        pub fn new(routes: Vec<(&'static str, Value)>) -> Self {
            FakeHttp { routes, calls: RefCell::new(vec![]) }
        }
        fn hit(&self, url: &str) -> Result<Value, String> {
            self.calls.borrow_mut().push(url.into());
            self.routes.iter().find(|(k, _)| url.contains(k)).map(|r| r.1.clone()).ok_or(format!("unrouted {url}"))
        }
    }
    impl Http for FakeHttp {
        fn get(&self, url: &str, h: &[(&str, String)]) -> Result<Value, String> {
            self.calls.borrow_mut().push(format!("hdr {h:?}"));
            self.hit(url)
        }
        fn post(&self, url: &str, b: &Value) -> Result<Value, String> {
            self.calls.borrow_mut().push(format!("body {b}"));
            self.hit(url)
        }
        fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
            // a route value "@<path>" serves that file's bytes
            self.hit(url).map(|v| match v.as_str().and_then(|p| p.strip_prefix('@')) {
                Some(p) => std::fs::read(p).unwrap(),
                None => b"JPEG".to_vec(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeHttp;
    use super::*;

    fn tmdb(h: &FakeHttp) -> Tmdb<'_> {
        let cfg = TmdbCfg { read_token: "tok".into(), base_url: "http://t".into(), image_base: "http://img".into(), language: "en-US".into(), ..Default::default() };
        Tmdb { cfg, http: h }
    }

    #[test]
    fn tmdb_movie_via_imdb_find() {
        let h = FakeHttp::new(vec![
            ("/find/tt0133093", json!({"movie_results":[{"id":603}],"tv_results":[]})),
            ("/movie/603", json!({"title":"The Matrix","release_date":"1999-03-30","overview":"Neo","genres":[{"name":"Action"}],"poster_path":"/p.jpg"})),
        ]);
        let m = tmdb(&h).lookup(&Query { imdb: Some("https://www.imdb.com/title/tt0133093/".into()), ..Default::default() }).unwrap().unwrap();
        assert_eq!((m["kind"].as_str(), m["title"].as_str(), m["year"].as_str()), ("movie", "The Matrix", "1999"));
        assert_eq!(m["poster_url"], "http://img/p.jpg");
        assert!(h.calls.borrow().iter().any(|c| c.contains("Bearer tok")));
    }

    #[test]
    fn tmdb_tv_search_and_episode() {
        let h = FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":42}]})),
            ("/tv/42/season/1?", json!({"poster_path":"/s1.jpg"})),
            ("/tv/42/season/1/episode/2", json!({"id":4202,"name":"Ep Two","air_date":"2016-07-11","overview":"o"})),
            ("/tv/42", json!({"name":"Saiki K","first_air_date":"2016-07-04","external_ids":{"imdb_id":"tt5"}})),
        ]);
        let q = Query { title: "Saiki K".into(), season: Some(1), episode: Some(2), ..Default::default() };
        let m = tmdb(&h).lookup(&q).unwrap().unwrap();
        assert_eq!((m["show"].as_str(), m["episode_title"].as_str(), m["season"].as_str()), ("Saiki K", "Ep Two", "1"));
        assert!(h.calls.borrow().iter().any(|c| c.contains("query=Saiki%20K")));
        assert_eq!(m["season_poster_url"], "http://img/s1.jpg");
        let mut t = tmdb(&h);
        t.cfg.poster_size = "w780".into();
        assert_eq!(t.img(&json!("/x.jpg")).unwrap(), "http://img/w780/x.jpg");
        assert_eq!((m["tmdb_series_id"].as_str(), m["tmdb_episode_id"].as_str(), m["imdb_id"].as_str()), ("42", "4202", "tt5"));
    }

    #[test]
    fn year_filters_in_search_queries() {
        let h = FakeHttp::new(vec![("/search/movie", json!({"results":[{"id":1}]})), ("/movie/1", json!({"title":"Dune"}))]);
        tmdb(&h).lookup(&Query { title: "Dune".into(), year: Some("1984".into()), ..Default::default() }).unwrap();
        assert!(h.calls.borrow().iter().any(|c| c.contains("/search/movie?") && c.contains("query=Dune&year=1984")), "{:?}", h.calls.borrow());
        let h = FakeHttp::new(vec![("/search/tv", json!({"results":[{"id":2}]})), ("/tv/2", json!({"name":"Cosmos"}))]);
        let q = Query { title: "Cosmos".into(), year: Some("1980".into()), season: Some(1), ..Default::default() };
        tmdb(&h).lookup(&q).unwrap();
        assert!(h.calls.borrow().iter().any(|c| c.contains("first_air_date_year=1980")));
        let h = FakeHttp::new(vec![("/login", json!({"data":{"token":"T"}})), ("/search", json!({"data":[{"tvdb_id":"9"}]})), ("/series/9", json!({"data":{"id":9,"name":"Cosmos"}}))]);
        Tvdb::new(TvdbCfg { api_key: "k".into(), base_url: "http://v".into(), ..Default::default() }, &h).lookup(&q).unwrap();
        assert!(h.calls.borrow().iter().any(|c| c.contains("type=series&query=Cosmos&year=1980")), "{:?}", h.calls.borrow());
    }

    #[test]
    fn tvdb_login_slug_episode_and_missing_keys() {
        let h = FakeHttp::new(vec![
            ("/login", json!({"data":{"token":"T"}})),
            ("/series/slug/the-disastrous-life", json!({"data":{"id":7,"name":"Saiki K","year":"2016"}})),
            ("/series/7/episodes", json!({"data":{"episodes":[{"name":"E","aired":"2016-07-04"}]}})),
            ("/series/7/extended", json!({"data":{"seasons":[{"number":1,"type":{"type":"dvd"},"image":"http://a/dvd.jpg"},{"number":1,"type":{"type":"official"},"image":"http://a/s1.jpg"}]}})),
        ]);
        let t = Tvdb::new(TvdbCfg { api_key: "k".into(), pin: "1234".into(), base_url: "http://v".into(), poster_size: "thumbnail".into() }, &h);
        let q = Query { thetvdb: Some("https://www.thetvdb.com/series/the-disastrous-life".into()), season: Some(1), episode: Some(1), ..Default::default() };
        let m = t.lookup(&q).unwrap().unwrap();
        assert_eq!((m["show"].as_str(), m["episode_title"].as_str()), ("Saiki K", "E"));
        assert_eq!(m["season_poster_url"], "http://a/s1_t.jpg");
        assert_eq!(h.calls.borrow().iter().filter(|c| c.contains("/login")).count(), 1);
        assert!(h.calls.borrow().iter().any(|c| c.contains("\"pin\":\"1234\"")));
        // movies are skipped; missing keys give clear errors
        assert!(t.lookup(&Query { title: "X".into(), ..Default::default() }).unwrap().is_none());
        let e = Tvdb::new(TvdbCfg::default(), &h).lookup(&q).unwrap_err();
        assert!(e.contains("[providers.thetvdb] api_key"));
        let mut tm = tmdb(&h);
        tm.cfg.read_token.clear();
        assert!(tm.lookup(&Query { title: "X".into(), ..Default::default() }).unwrap_err().contains("[providers.tmdb]"));
    }
}
