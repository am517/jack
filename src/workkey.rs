//! Work-level key: groups versions of the same work (encodes, releases, cuts) across files.
//! Built from a canonical reference string in APA 7 style, normalized and hashed. Only stable parts
//! are used (no episode titles, people or studios: sources word those differently), so every release
//! of a work yields the same reference. Ids are kept alongside (works.ids), not in the key, so a copy
//! known only by name and year still lands in the same group as one known by id.
use crate::meta::Meta;
use crate::resolver::norm;

fn get<'a>(m: &'a Meta, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.trim()).filter(|s| !s.is_empty())
}

fn year(m: &Meta, k: &str) -> Option<String> {
    get(m, k)
        .map(|y| y.chars().take(4).collect())
        .filter(|y: &String| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()))
}

/// APA 7 style reference, or None when there is not enough to name the work.
pub fn reference(m: &Meta) -> Option<String> {
    match get(m, "kind").unwrap_or("movie") {
        "episode" => {
            let show = get(m, "show")?;
            let (s, e) = (
                get(m, "season")?.parse::<u32>().ok()?,
                get(m, "episode")?.parse::<u32>().ok()?,
            );
            let y = year(m, "series_year").or(year(m, "year"))?;
            Some(format!(
                "{show} (Season {s}, Episode {e}) [TV series episode]. ({y})."
            ))
        }
        "show" => Some(format!(
            "{}. ({}). [TV series].",
            get(m, "show").or(get(m, "title"))?,
            year(m, "year")?
        )),
        crate::online::KIND => {
            let date = get(m, "date")?;
            Some(format!(
                "{}. ({date}). {} [Video]. {}.",
                get(m, "channel")?,
                get(m, "title")?,
                get(m, "site").unwrap_or("web")
            ))
        }
        "unknown" => None,
        _ => Some(format!(
            "{}. ({}). [Film].",
            get(m, "title")?,
            year(m, "year")?
        )),
    }
}

/// `wk1:` + BLAKE3 of the normalized reference.
pub fn key(reference: &str) -> String {
    format!("wk1:{}", blake3::hash(norm(reference).as_bytes()).to_hex())
}

/// Work-level ids (not file or episode-run specific), as JSON for works.ids.
pub fn ids(m: &Meta) -> String {
    let ids: std::collections::BTreeMap<&str, &str> = [
        "imdb_id",
        "tmdb_id",
        "thetvdb_id",
        "tvmaze_id",
        "wikidata_id",
        "youtube_video_id",
        "x_video_id",
    ]
    .iter()
    .filter_map(|k| get(m, k).map(|v| (*k, v)))
    .collect();
    serde_json::to_string(&ids).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(kv: &[(&str, &str)]) -> Meta {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
    #[test]
    fn same_work_same_key_across_releases() {
        let a = m(&[
            ("kind", "movie"),
            ("title", "The Matrix"),
            ("year", "1999"),
            ("imdb_id", "tt0133093"),
        ]);
        let b = m(&[
            ("kind", "movie"),
            ("title", "the matrix"),
            ("year", "1999-03-31"),
        ]);
        assert_eq!(reference(&a).unwrap(), "The Matrix. (1999). [Film].");
        assert_eq!(key(&reference(&a).unwrap()), key(&reference(&b).unwrap()));
        let c = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "2003")]);
        assert_ne!(key(&reference(&a).unwrap()), key(&reference(&c).unwrap()));
        let e = m(&[
            ("kind", "episode"),
            ("show", "The Office"),
            ("season", "01"),
            ("episode", "1"),
            ("series_year", "2005"),
            ("episode_title", "Pilot"),
        ]);
        assert_eq!(
            reference(&e).unwrap(),
            "The Office (Season 1, Episode 1) [TV series episode]. (2005)."
        );
        assert_eq!(reference(&m(&[("kind", "unknown"), ("title", "x")])), None);
        assert_eq!(reference(&m(&[("title", "No Year")])), None);
        assert_eq!(ids(&a), r#"{"imdb_id":"tt0133093"}"#);
    }
}
