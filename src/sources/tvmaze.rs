//! TVmaze: free TV show and episode data (no key).
use crate::meta::Meta;
use crate::providers::{Http, Provider, Query, enc, put, s};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct TvmazeCfg {
    pub base_url: String,
}

pub struct Tvmaze<'a> {
    pub cfg: TvmazeCfg,
    pub http: &'a dyn Http,
}

/// TVmaze summaries are HTML; keep the text.
fn text(v: &Value) -> Option<String> {
    let h = s(v)?;
    let t = regex::Regex::new(r"<[^>]*>")
        .unwrap()
        .replace_all(&h, "")
        .trim()
        .to_string();
    (!t.is_empty()).then_some(t)
}

impl Tvmaze<'_> {
    fn show(&self, q: &Query) -> Result<Value, String> {
        let b = &self.cfg.base_url;
        let url = match (&q.imdb, &q.thetvdb) {
            (Some(i), _) => format!("{b}/lookup/shows?imdb={}", enc(i)),
            (_, Some(t)) => format!("{b}/lookup/shows?thetvdb={}", enc(t)),
            _ => format!("{b}/singlesearch/shows?q={}", enc(&q.title)),
        };
        self.http.get(&url, &[])
    }
}

impl Provider for Tvmaze<'_> {
    fn name(&self) -> &'static str {
        "tvmaze"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        if !q.is_tv() {
            return Ok(None);
        }
        let sh = self.show(q)?;
        let Some(id) = s(&sh["id"]) else {
            return Ok(None);
        };
        let mut m = Meta::new();
        m.insert("tvmaze_id".into(), id.clone());
        put(&mut m, "show", s(&sh["name"]));
        put(
            &mut m,
            "year",
            s(&sh["premiered"]).map(|d| d.chars().take(4).collect()),
        );
        put(&mut m, "poster_url", s(&sh["image"]["original"]));
        put(&mut m, "imdb_id", s(&sh["externals"]["imdb"]));
        put(&mut m, "thetvdb_series_id", s(&sh["externals"]["thetvdb"]));
        let g: Vec<String> = sh["genres"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(s)
            .collect();
        put(&mut m, "genre", (!g.is_empty()).then(|| g.join(",")));
        match (q.season, q.episode) {
            (Some(se), Some(ep)) => {
                let e = self.http.get(
                    &format!(
                        "{}/shows/{id}/episodebynumber?season={se}&number={ep}",
                        self.cfg.base_url
                    ),
                    &[],
                )?;
                m.insert("kind".into(), "episode".into());
                m.insert("season".into(), se.to_string());
                m.insert("episode".into(), ep.to_string());
                put(&mut m, "tvmaze_episode_id", s(&e["id"]));
                put(&mut m, "episode_title", s(&e["name"]));
                put(&mut m, "still_url", s(&e["image"]["original"]));
                put(&mut m, "description", text(&e["summary"]));
            }
            _ => put(&mut m, "description", text(&sh["summary"])),
        }
        Ok(Some(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use serde_json::json;

    #[test]
    fn show_and_episode_by_imdb() {
        let h = FakeHttp::new(vec![
            (
                "/lookup/shows?imdb=tt1",
                json!({"id":82,"name":"Saiki K","premiered":"2016-07-04","genres":["Comedy","Anime"],"image":{"original":"http://i/p.jpg"},"externals":{"imdb":"tt1","thetvdb":305074},"summary":"<p>Show</p>"}),
            ),
            (
                "/shows/82/episodebynumber?season=1&number=2",
                json!({"id":900,"name":"Ep 2","summary":"<p><b>Psychic</b> trouble.</p>","image":{"original":"http://i/s.jpg"}}),
            ),
        ]);
        let t = Tvmaze {
            cfg: TvmazeCfg {
                base_url: "http://tm".into(),
            },
            http: &h,
        };
        let q = Query {
            imdb: Some("tt1".into()),
            season: Some(1),
            episode: Some(2),
            kind: Some("tv".into()),
            ..Default::default()
        };
        let m = t.lookup(&q).unwrap().unwrap();
        assert_eq!(
            (
                m["tvmaze_id"].as_str(),
                m["thetvdb_series_id"].as_str(),
                m["year"].as_str()
            ),
            ("82", "305074", "2016")
        );
        assert_eq!(
            (
                m["episode_title"].as_str(),
                m["description"].as_str(),
                m["genre"].as_str()
            ),
            ("Ep 2", "Psychic trouble.", "Comedy,Anime")
        );
        assert!(
            t.lookup(&Query {
                kind: Some("movie".into()),
                ..Default::default()
            })
            .unwrap()
            .is_none()
        );
    }
}
