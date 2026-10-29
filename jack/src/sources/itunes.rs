//! iTunes Search API: high-resolution art (no key).
use crate::meta::Meta;
use crate::providers::{Http, Provider, Query, enc, put, s};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct ItunesCfg {
    pub base_url: String,
    pub country: String,
    /// Replaces "100x100bb" in artwork URLs, e.g. "3000x3000bb".
    pub art_size: String,
    pub limit: u32,
}

pub struct Itunes<'a> {
    pub cfg: ItunesCfg,
    pub http: &'a dyn Http,
}

impl Itunes<'_> {
    fn pick<'v>(&self, q: &Query, results: &'v [Value]) -> Option<&'v Value> {
        let want = |r: &&Value| match (q.is_tv(), q.season, &q.year) {
            (true, Some(n), _) => {
                s(&r["collectionName"]).is_some_and(|c| c.contains(&format!("Season {n}")))
            }
            (false, _, Some(y)) => s(&r["releaseDate"]).is_some_and(|d| d.starts_with(y.as_str())),
            _ => true,
        };
        results.iter().find(want).or(results.first())
    }
}

impl Provider for Itunes<'_> {
    fn name(&self) -> &'static str {
        "itunes"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        let (media, entity) = if q.is_tv() {
            ("tvShow", "tvSeason")
        } else {
            ("movie", "movie")
        };
        let url = format!(
            "{}?term={}&media={media}&entity={entity}&country={}&limit={}",
            self.cfg.base_url,
            enc(&q.title),
            enc(&self.cfg.country),
            self.cfg.limit
        );
        let r = self.http.get(&url, &[])?;
        let Some(hit) = self.pick(
            q,
            r["results"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
        ) else {
            return Ok(None);
        };
        let mut m = Meta::new();
        put(
            &mut m,
            "itunes_id",
            s(&hit["trackId"]).or(s(&hit["collectionId"])),
        );
        let art = s(&hit["artworkUrl100"]).map(|u| u.replace("100x100bb", &self.cfg.art_size));
        put(
            &mut m,
            if q.is_tv() {
                "season_poster_url"
            } else {
                "poster_url"
            },
            art,
        );
        Ok(Some(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use serde_json::json;

    #[test]
    fn movie_art_by_year_and_tv_season_art() {
        let h = FakeHttp::new(vec![
            (
                "media=movie",
                json!({"results":[{"trackId":1,"releaseDate":"2021-01-01","artworkUrl100":"http://a/x/100x100bb.jpg"},{"trackId":2,"releaseDate":"1999-03-31","artworkUrl100":"http://a/y/100x100bb.jpg"}]}),
            ),
            (
                "media=tvShow",
                json!({"results":[{"collectionId":5,"collectionName":"Saiki K, Season 1"},{"collectionId":6,"collectionName":"Saiki K, Season 2","artworkUrl100":"http://a/s2/100x100bb.jpg"}]}),
            ),
        ]);
        let i = Itunes {
            cfg: ItunesCfg {
                base_url: "http://it".into(),
                country: "US".into(),
                art_size: "3000x3000bb".into(),
                limit: 5,
            },
            http: &h,
        };
        let m = i
            .lookup(&Query {
                title: "The Matrix".into(),
                year: Some("1999".into()),
                kind: Some("movie".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (m["itunes_id"].as_str(), m["poster_url"].as_str()),
            ("2", "http://a/y/3000x3000bb.jpg")
        );
        let m = i
            .lookup(&Query {
                title: "Saiki K".into(),
                season: Some(2),
                kind: Some("tv".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            (m["itunes_id"].as_str(), m["season_poster_url"].as_str()),
            ("6", "http://a/s2/3000x3000bb.jpg")
        );
    }
}
