//! Wikidata: cross-ids, franchise and series groupings, and the Wikipedia article title.
//! Looked up only by an id we already know (no title guessing).
use crate::meta::Meta;
use crate::providers::{Http, Provider, Query, enc, put, s};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct WikidataCfg {
    pub base_url: String,
    /// Label language, also picks the Wikipedia site ("<language>wiki").
    pub language: String,
}

pub struct Wikidata<'a> {
    pub cfg: WikidataCfg,
    pub http: &'a dyn Http,
}

/// Meta key -> Wikidata property, for string-valued ids.
const IDS: [(&str, &str); 5] = [
    ("imdb_id", "P345"),
    ("tmdb_id", "P4947"),
    ("tmdb_series_id", "P4983"),
    ("thetvdb_series_id", "P4835"),
    ("tvmaze_id", "P8600"),
];
/// Meta key -> Wikidata property, for item-valued groupings.
const GROUPS: [(&str, &str); 2] = [("franchise", "P8345"), ("series_group", "P179")];

fn claim(e: &Value, p: &str) -> Option<String> {
    let v = &e["claims"][p][0]["mainsnak"]["datavalue"]["value"];
    s(v).or_else(|| s(&v["id"]))
}

impl Wikidata<'_> {
    fn api(&self, q: &str) -> Result<Value, String> {
        self.http
            .get(&format!("{}?format=json&{q}", self.cfg.base_url), &[])
    }

    /// The item id for the first known id, e.g. haswbstatement:P345=tt0133093.
    fn find(&self, q: &Query) -> Result<Option<String>, String> {
        let tmdb = q.tmdb.as_deref().map(|t| match t.strip_prefix("/tv/") {
            Some(x) => ("P4983", x.to_string()),
            None => ("P4947", t.trim_start_matches("/movie/").to_string()),
        });
        let key = q
            .imdb
            .clone()
            .map(|i| ("P345", i))
            .or(tmdb)
            .or(q.thetvdb.clone().map(|t| ("P4835", t)));
        let Some((p, v)) = key else { return Ok(None) };
        let r = self.api(&format!(
            "action=query&list=search&srlimit=1&srsearch={}",
            enc(&format!("haswbstatement:{p}={v}"))
        ))?;
        Ok(s(&r["query"]["search"][0]["title"]))
    }

    fn labels(&self, ids: &[String]) -> Result<Value, String> {
        self.api(&format!(
            "action=wbgetentities&props=labels&languages={}&ids={}",
            enc(&self.cfg.language),
            enc(&ids.join("|"))
        ))
    }
}

impl Provider for Wikidata<'_> {
    fn name(&self) -> &'static str {
        "wikidata"
    }
    fn lookup(&self, q: &Query) -> Result<Option<Meta>, String> {
        let Some(qid) = self.find(q)? else {
            return Ok(None);
        };
        let r = self.api(&format!(
            "action=wbgetentities&props=claims%7Csitelinks&ids={}",
            enc(&qid)
        ))?;
        let e = &r["entities"][&qid];
        let mut m = Meta::new();
        m.insert("wikidata_id".into(), qid.clone());
        for (k, p) in IDS {
            put(&mut m, k, claim(e, p));
        }
        put(
            &mut m,
            "wikipedia_title",
            s(&e["sitelinks"][format!("{}wiki", self.cfg.language)]["title"]),
        );
        let g: Vec<(&str, String)> = GROUPS
            .iter()
            .filter_map(|(k, p)| claim(e, p).map(|id| (*k, id)))
            .collect();
        if !g.is_empty() {
            let l = self.labels(&g.iter().map(|x| x.1.clone()).collect::<Vec<_>>())?;
            for (k, id) in g {
                put(
                    &mut m,
                    k,
                    s(&l["entities"][&id]["labels"][&self.cfg.language]["value"]),
                );
            }
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
    fn ids_groups_and_wikipedia_title_from_known_imdb() {
        let item = |v: &str| json!([{"mainsnak":{"datavalue":{"value":{"id":v}}}}]);
        let st = |v: &str| json!([{"mainsnak":{"datavalue":{"value":v}}}]);
        let h = FakeHttp::new(vec![
            (
                "srsearch=haswbstatement%3AP345%3Dtt0133093",
                json!({"query":{"search":[{"title":"Q83495"}]}}),
            ),
            (
                "props=claims",
                json!({"entities":{"Q83495":{"claims":{"P4947":st("603"),"P8345":item("Q1"),"P179":item("Q2")},"sitelinks":{"enwiki":{"title":"The Matrix"}}}}}),
            ),
            (
                "props=labels",
                json!({"entities":{"Q1":{"labels":{"en":{"value":"The Matrix franchise"}}},"Q2":{"labels":{"en":{"value":"The Matrix trilogy"}}}}}),
            ),
        ]);
        let w = Wikidata {
            cfg: WikidataCfg {
                base_url: "http://w".into(),
                language: "en".into(),
            },
            http: &h,
        };
        let m = w
            .lookup(&Query {
                imdb: Some("tt0133093".into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(m["wikidata_id"], "Q83495");
        assert_eq!(m["tmdb_id"], "603");
        assert_eq!(
            (m["franchise"].as_str(), m["series_group"].as_str()),
            ("The Matrix franchise", "The Matrix trilogy")
        );
        assert_eq!(m["wikipedia_title"], "The Matrix");
        assert!(
            w.lookup(&Query {
                title: "x".into(),
                ..Default::default()
            })
            .unwrap()
            .is_none(),
            "no id: no guessing"
        );
    }
}
