//! Where sources about the same work disagree. Found by logic, recorded, never settled by assumption.
use crate::meta::Meta;

/// Compared fields; "year" is compared by its first four digits.
const FIELDS: [&str; 7] = crate::facts::FACTS;

/// (field, [(source, value)]) where at least two sources differ (ignoring case/spacing/separators).
/// Only sources about the chosen work count: same title as the chosen one, or the file itself.
pub fn find(chosen: &Meta, found: &[(String, Meta)]) -> Vec<(String, Vec<(String, String)>)> {
    let name = chosen.get("show").or(chosen.get("title")).map_or("", |s| s);
    let same: Vec<&(String, Meta)> = found
        .iter()
        .filter(|(s, _)| !crate::candidates::is_search(s))
        .filter(|(s, m)| {
            s == "embedded" || crate::resolver::exact(name, None, m) || shares_id(chosen, m)
        })
        .filter(|(_, m)| m.get("kind").is_none_or(|k| Some(k) == chosen.get("kind")))
        .collect();
    let mut out = vec![];
    for f in FIELDS {
        if chosen.get("kind").is_some_and(|k| k == "episode") && (f == "title" || f == "year") {
            continue; // an episode's title/year mix show and episode values across sources
        }
        let vals: Vec<(String, String)> = same
            .iter()
            .filter_map(|(s, m)| {
                m.get(f).map(|v| {
                    (
                        s.clone(),
                        if f == "year" {
                            v.chars().take(4).collect()
                        } else {
                            v.clone()
                        },
                    )
                })
            })
            .collect();
        // names differing beyond formatting clash unless independent evidence confirmed the entity
        let confirmed = chosen.contains_key("evidence") && !crate::facts::EXACT.contains(&f);
        let clash = !confirmed
            && vals
                .iter()
                .any(|(_, a)| vals.iter().any(|(_, b)| !crate::facts::same(f, a, b)));
        if clash {
            out.push((f.to_string(), vals));
        }
    }
    out
}

/// Same work by id (any *_id both carry with the same value).
fn shares_id(a: &Meta, b: &Meta) -> bool {
    b.iter()
        .any(|(k, v)| k.ends_with("_id") && a.get(k) == Some(v))
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
    fn records_disagreement_ignores_other_works() {
        let chosen = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]);
        let found = vec![
            (
                "tmdb".into(),
                m(&[("title", "The Matrix"), ("year", "1999-03-31")]),
            ),
            (
                "omdb".into(),
                m(&[("title", "the.matrix"), ("year", "1999")]),
            ),
            (
                "itunes".into(),
                m(&[("title", "The Matrix Reloaded"), ("year", "2003")]),
            ),
            (
                "embedded".into(),
                m(&[("title", "Matrix"), ("year", "1998")]),
            ),
        ];
        let c = find(&chosen, &found);
        // "Matrix" vs "The Matrix" is not proven the same: recorded; 1998 vs 1999 too
        assert_eq!(
            c.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
            ["title", "year"]
        );
        assert!(!c.iter().any(|x| x.1.iter().any(|(s, _)| s == "itunes")));
        assert!(find(&chosen, &found[..2]).is_empty());
        // with evidence, a fuller name is not a conflict; a different year still is
        let chosen = m(&[
            ("kind", "online_video"),
            ("show", "The Joe Rogan Experience"),
            ("evidence", "id"),
        ]);
        let f = vec![
            (
                "tmdb".into(),
                m(&[("show", "The Joe Rogan Experience"), ("year", "2009")]),
            ),
            (
                "embedded".into(),
                m(&[("show", "Joe Rogan"), ("year", "2010")]),
            ),
        ];
        assert_eq!(
            find(&chosen, &f)
                .iter()
                .map(|x| x.0.as_str())
                .collect::<Vec<_>>(),
            ["year"]
        );
        // without evidence it is recorded; search lists never count
        let chosen = m(&[
            ("kind", "online_video"),
            ("show", "Joe Rogan"),
            ("tmdb_series_id", "9"),
        ]);
        let f = vec![
            (
                "tmdb".into(),
                m(&[("show", "Monica Rogan"), ("tmdb_series_id", "9")]),
            ),
            (
                "tmdb:search".into(),
                m(&[("show", "Joe Rogan"), ("year", "1")]),
            ),
            ("embedded".into(), m(&[("show", "Joe Rogan")])),
        ];
        assert_eq!(
            find(&chosen, &f)
                .iter()
                .map(|x| x.0.as_str())
                .collect::<Vec<_>>(),
            ["show"]
        );
    }
}
