//! Ruth, the reference windup: builds the APA 7 work reference and the work key that groups
//! versions of the same work (encodes, releases, cuts). Built from the most original information:
//! original-language titles (original_title / original_show / original_episode_title) and the
//! original release year, never a translation; the episode title is included when known. Whenever
//! the reference can't be fully built (no original title, no original language, no episode title,
//! ...) it is *provisional*; the work's permanent local id (works.id) stays while it improves.
//! Ruth only reads gathered facts; Jack gathers them, Daniel stores the result.
use std::collections::BTreeMap;

pub type Meta = BTreeMap<String, String>;

pub const PROFILE: &str = include_str!("../PROFILE.toml");
const ONLINE: &str = "online_video";

fn get<'a>(m: &'a Meta, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.trim()).filter(|s| !s.is_empty())
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['.', '_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads facts and notes whatever had to fall back (which makes the reference provisional).
struct Facts<'a> {
    m: &'a Meta,
    missing: std::cell::RefCell<Vec<String>>,
}

impl<'a> Facts<'a> {
    /// The original-language value; the display one (noted as missing the original) otherwise.
    fn orig(&self, k: &str) -> Option<&'a str> {
        get(self.m, &format!("original_{k}")).or_else(|| {
            let v = get(self.m, k);
            if v.is_some() {
                self.missing.borrow_mut().push(format!("original_{k}"));
            }
            v
        })
    }
}

fn year(m: &Meta, k: &str) -> Option<String> {
    get(m, k)
        .map(|y| y.chars().take(4).collect())
        .filter(|y: &String| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()))
}

pub struct Reference {
    pub text: String,
    /// True whenever it couldn't be fully built; `missing` says what was lacking.
    pub provisional: bool,
    pub missing: Vec<String>,
}

/// APA 7 style reference, or None when there is not enough to name the work.
pub fn reference(m: &Meta) -> Option<Reference> {
    let f = Facts {
        m,
        missing: Default::default(),
    };
    let text = match get(m, "kind").unwrap_or("movie") {
        "episode" => {
            let show = f.orig("show")?;
            let (s, e) = (
                get(m, "season")?.parse::<u32>().ok()?,
                get(m, "episode")?.parse::<u32>().ok()?,
            );
            let y = year(m, "series_year").or(year(m, "year"))?;
            match get(m, "original_episode_title").or(get(m, "episode_title")) {
                Some(t) => {
                    if get(m, "original_episode_title").is_none() {
                        f.missing.borrow_mut().push("original_episode_title".into());
                    }
                    format!("{t} (Season {s}, Episode {e}) [TV series episode]. In {show}. ({y}).")
                }
                None => {
                    f.missing.borrow_mut().push("episode_title".into());
                    format!("{show} (Season {s}, Episode {e}) [TV series episode]. ({y}).")
                }
            }
        }
        "show" => {
            let t = f.orig("show").or_else(|| f.orig("title"))?;
            format!("{t}. ({}). [TV series].", year(m, "year")?)
        }
        ONLINE => {
            // an online video's title is as published: already the original
            let date = get(m, "date")?;
            let t = format!(
                "{}. ({date}). {} [Video]. {}.",
                get(m, "channel")?,
                get(m, "title")?,
                get(m, "site").unwrap_or("web")
            );
            return Some(Reference {
                text: t,
                provisional: false,
                missing: vec![],
            });
        }
        "unknown" => return None,
        _ => format!("{}. ({}). [Film].", f.orig("title")?, year(m, "year")?),
    };
    let mut missing = f.missing.into_inner();
    if get(m, "original_language").is_none() {
        missing.push("original_language".into());
    }
    Some(Reference {
        text,
        provisional: !missing.is_empty(),
        missing,
    })
}

/// `wk1:` + BLAKE3 of the normalized reference.
pub fn key(reference: &str) -> String {
    format!("wk1:{}", blake3::hash(norm(reference).as_bytes()).to_hex())
}

/// Work-level per-service ids (not file or episode-run specific).
pub fn ids(m: &Meta) -> Vec<(String, String)> {
    [
        "imdb_id",
        "tmdb_id",
        "thetvdb_id",
        "tvmaze_id",
        "wikidata_id",
        "youtube_video_id",
        "x_video_id",
    ]
    .iter()
    .filter_map(|k| get(m, k).map(|v| (k.to_string(), v.to_string())))
    .collect()
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
    fn original_names_episode_titles_and_provisional() {
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
        let ra = reference(&a).unwrap();
        assert_eq!(ra.text, "The Matrix. (1999). [Film].");
        assert!(ra.provisional, "original language not known yet");
        assert_eq!(key(&ra.text), key(&reference(&b).unwrap().text));
        let c = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "2003")]);
        assert_ne!(key(&ra.text), key(&reference(&c).unwrap().text));
        // a translated title gives way to the original
        let s = m(&[
            ("kind", "movie"),
            ("title", "Spirited Away"),
            ("original_title", "千と千尋の神隠し"),
            ("original_language", "ja"),
            ("year", "2001"),
        ]);
        let rs = reference(&s).unwrap();
        assert_eq!(rs.text, "千と千尋の神隠し. (2001). [Film].");
        assert!(!rs.provisional);
        // a display title alone isn't enough, even with the language
        let half = m(&[
            ("kind", "movie"),
            ("title", "Heat"),
            ("original_language", "en"),
            ("year", "1995"),
        ]);
        let rh = reference(&half).unwrap();
        assert!(
            rh.provisional && rh.missing == ["original_title"],
            "{:?}",
            rh.missing
        );
        let e = m(&[
            ("kind", "episode"),
            ("show", "The Office"),
            ("season", "01"),
            ("episode", "1"),
            ("series_year", "2005"),
            ("episode_title", "Pilot"),
            ("original_language", "en"),
        ]);
        assert!(
            reference(&e).unwrap().provisional,
            "no original show/episode title"
        );
        assert_eq!(
            reference(&e).unwrap().text,
            "Pilot (Season 1, Episode 1) [TV series episode]. In The Office. (2005)."
        );
        assert!(PROFILE.contains("name = \"ruth\""));
        assert!(reference(&m(&[("kind", "unknown"), ("title", "x")])).is_none());
        assert!(reference(&m(&[("title", "No Year")])).is_none());
        assert_eq!(ids(&a), [("imdb_id".to_string(), "tt0133093".to_string())]);
    }
}
