//! Work reference and key: groups versions of the same work (encodes, releases, cuts).
//! The reference is APA 7 style and built from the most original information: the original-language
//! title (original_title / original_show / original_episode_title when sources give them) and the
//! original release year, never a translation. The episode title is included when known. Until the
//! original language is known the reference is *provisional*; the work's permanent local id
//! (works.id) stays while its reference improves. Per-service ids are kept beside it (work_ids).
use crate::meta::Meta;
use crate::resolver::norm;

fn get<'a>(m: &'a Meta, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// The original-language value if a source gave one, else the display one.
fn orig<'a>(m: &'a Meta, k: &str) -> Option<&'a str> {
    get(m, &format!("original_{k}")).or(get(m, k))
}

fn year(m: &Meta, k: &str) -> Option<String> {
    get(m, k)
        .map(|y| y.chars().take(4).collect())
        .filter(|y: &String| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()))
}

pub struct Reference {
    pub text: String,
    /// True until the original language is known (names may still be translations).
    pub provisional: bool,
}

/// APA 7 style reference, or None when there is not enough to name the work.
pub fn reference(m: &Meta) -> Option<Reference> {
    let known_lang = get(m, "original_language").is_some();
    let text = match get(m, "kind").unwrap_or("movie") {
        "episode" => {
            let show = orig(m, "show")?;
            let (s, e) = (
                get(m, "season")?.parse::<u32>().ok()?,
                get(m, "episode")?.parse::<u32>().ok()?,
            );
            let y = year(m, "series_year").or(year(m, "year"))?;
            match orig(m, "episode_title") {
                Some(t) => {
                    format!("{t} (Season {s}, Episode {e}) [TV series episode]. In {show}. ({y}).")
                }
                None => format!("{show} (Season {s}, Episode {e}) [TV series episode]. ({y})."),
            }
        }
        "show" => format!(
            "{}. ({}). [TV series].",
            orig(m, "show").or(orig(m, "title"))?,
            year(m, "year")?
        ),
        crate::online::KIND => {
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
            });
        }
        "unknown" => return None,
        _ => format!("{}. ({}). [Film].", orig(m, "title")?, year(m, "year")?),
    };
    Some(Reference {
        text,
        provisional: !known_lang,
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
        let e = m(&[
            ("kind", "episode"),
            ("show", "The Office"),
            ("season", "01"),
            ("episode", "1"),
            ("series_year", "2005"),
            ("episode_title", "Pilot"),
            ("original_language", "en"),
        ]);
        assert_eq!(
            reference(&e).unwrap().text,
            "Pilot (Season 1, Episode 1) [TV series episode]. In The Office. (2005)."
        );
        assert!(reference(&m(&[("kind", "unknown"), ("title", "x")])).is_none());
        assert!(reference(&m(&[("title", "No Year")])).is_none());
        assert_eq!(ids(&a), [("imdb_id".to_string(), "tt0133093".to_string())]);
    }
}
