//! Every hit a search returned, kept as a candidate. One API call per source: the search list
//! comes back in the same call that picks the result, so keeping all of it costs nothing.
use crate::meta::Meta;
use crate::providers::s;
use serde_json::Value;

/// Source name for a search list ("tmdb" -> "tmdb:search"): stored, never merged or compared.
pub fn source(provider: &str) -> String {
    format!("{provider}:search")
}

pub fn is_search(source: &str) -> bool {
    source.ends_with(":search")
}

fn hit(
    name: Option<String>,
    date: Option<String>,
    ids: &[(&str, Option<String>)],
    tv: bool,
) -> Option<Meta> {
    let name = name.filter(|n| !n.is_empty())?;
    let mut m = Meta::new();
    m.insert("kind".into(), if tv { "show" } else { "movie" }.into());
    m.insert("title".into(), name.clone());
    if tv {
        m.insert("show".into(), name);
    }
    if let Some(y) = date
        .map(|d| d.chars().take(4).collect::<String>())
        .filter(|y| y.len() == 4)
    {
        m.insert("year".into(), y);
    }
    for (k, v) in ids {
        if let Some(v) = v {
            m.insert(k.to_string(), v.clone());
        }
    }
    Some(m)
}

/// TMDB /search/tv or /search/movie: `results`.
pub fn tmdb(r: &Value, tv: bool) -> Vec<Meta> {
    let list = r["results"].as_array().into_iter().flatten();
    list.filter_map(|x| {
        let id = s(&x["id"]);
        let ids = if tv {
            [("tmdb_series_id", id.clone()), ("tmdb_id", id)]
        } else {
            [("tmdb_id", id), ("", None)]
        };
        let (n, d) = if tv {
            ("name", "first_air_date")
        } else {
            ("title", "release_date")
        };
        hit(s(&x[n]), s(&x[d]), &ids, tv)
    })
    .collect()
}

/// TheTVDB /search: `data`.
pub fn tvdb(r: &Value, tv: bool) -> Vec<Meta> {
    let list = r.as_array().into_iter().flatten();
    list.filter_map(|x| {
        hit(
            s(&x["name"]),
            s(&x["year"]),
            &[("thetvdb_id", s(&x["tvdb_id"]))],
            tv,
        )
    })
    .collect()
}

/// TVmaze /search/shows: `[{show}]`.
pub fn tvmaze(r: &Value) -> Vec<Meta> {
    let list = r.as_array().into_iter().flatten().map(|h| &h["show"]);
    list.filter_map(|x| {
        hit(
            s(&x["name"]),
            s(&x["premiered"]),
            &[("tvmaze_id", s(&x["id"]))],
            true,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn whole_lists_kept() {
        let r = json!({"results":[{"id":1,"name":"The Office","first_air_date":"2005-03-24"},{"id":2,"name":"The Office","first_air_date":"2001-07-09"},{"id":3}]});
        let c = tmdb(&r, true);
        assert_eq!(c.len(), 2);
        assert_eq!(
            (
                c[1]["show"].as_str(),
                c[1]["year"].as_str(),
                c[1]["tmdb_series_id"].as_str()
            ),
            ("The Office", "2001", "2")
        );
        assert_eq!(
            tmdb(
                &json!({"results":[{"id":5,"title":"Dune","release_date":"1984-12-14"}]}),
                false
            )[0]["tmdb_id"],
            "5"
        );
        assert_eq!(
            tvdb(&json!([{"name":"X","year":"2020","tvdb_id":"9"}]), true)[0]["thetvdb_id"],
            "9"
        );
        assert_eq!(
            tvmaze(&json!([{"show":{"name":"Doctor Who","premiered":"1963-11-23","id":7}}]))[0]["year"],
            "1963"
        );
        assert!(is_search(&source("tmdb")) && !is_search("tmdb"));
    }
}
