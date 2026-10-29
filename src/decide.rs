//! Is the information enough to act on? One call per source returned whole search lists; a
//! title without a year is a hit only when exactly one exact match exists across all of them and
//! the sources agree. Otherwise: unresolved, with a reason and what would help.
use crate::candidates::is_search;
use crate::config::Settings;
use crate::meta::Meta;
use crate::providers::Query;
use crate::resolver::{exact, norm};

#[derive(Debug, PartialEq)]
pub enum Decision {
    Sure,
    /// (reason from unresolved_reasons, detail)
    Unresolved(&'static str, String),
}

fn year(m: &Meta) -> Option<String> {
    m.get("series_year")
        .or(m.get("year"))
        .map(|y| y.chars().take(4).collect())
}

fn label(m: &Meta) -> String {
    let t = m.get("show").or(m.get("title")).map_or("?", |s| s);
    format!("{t} ({})", year(m).unwrap_or_else(|| "?".into()))
}

pub fn decide(q: &Query, s: &Settings, found: &[(String, Meta)], year_tolerance: u32) -> Decision {
    let set = |o: &Option<String>| o.as_ref().is_some_and(|v| !v.is_empty());
    if q.imdb.is_some() || q.tmdb.is_some() || q.thetvdb.is_some() || (set(&s.name) && set(&s.year))
    {
        return Decision::Sure;
    }
    let is_hit = |m: &Meta| {
        let mut m = m.clone();
        if let Some(y) = year(&m) {
            m.insert("year".into(), y);
        }
        exact(&q.title, q.year.as_deref(), &m)
    };
    let found: Vec<&(String, Meta)> = found.iter().filter(|(src, _)| src != "embedded").collect();
    let hits: Vec<&(String, Meta)> = found.iter().copied().filter(|(_, m)| is_hit(m)).collect();
    if hits.is_empty() {
        return Decision::Unresolved("no_match", format!("no source has exactly \"{}\"", q.title));
    }
    // one source listing several exact matches: they can't be told apart from here
    let mut srcs: Vec<&str> = hits
        .iter()
        .filter(|(src, _)| is_search(src))
        .map(|(src, _)| src.as_str())
        .collect();
    srcs.sort();
    if let Some(w) = srcs.windows(2).find(|w| w[0] == w[1]) {
        let list: Vec<String> = hits
            .iter()
            .filter(|(src, _)| src == w[0])
            .map(|(_, m)| label(m))
            .collect();
        return Decision::Unresolved(
            "ambiguous",
            format!("\"{}\" could be {}", q.title, list.join(" / ")),
        );
    }
    // sources naming different works (different years) disagree
    let mut years: Vec<String> = hits.iter().filter_map(|(_, m)| year(m)).collect();
    let nums: Vec<i64> = years.iter().filter_map(|y| y.parse().ok()).collect();
    let spread = nums
        .iter()
        .max()
        .zip(nums.iter().min())
        .map_or(0, |(a, b)| a - b);
    years.sort();
    years.dedup();
    if years.len() > 1 && (nums.len() < years.len() || spread > year_tolerance as i64) {
        let list: Vec<String> = hits
            .iter()
            .map(|(src, m)| format!("{src}: {}", label(m)))
            .collect();
        return Decision::Unresolved(
            "needs_flags",
            format!("sources disagree on \"{}\": {}", q.title, list.join(", ")),
        );
    }
    // the details fetched must be about the exact match, not about some other top result
    let chosen: Vec<&(String, Meta)> = found
        .iter()
        .copied()
        .filter(|(src, _)| !is_search(src))
        .collect();
    if let Some((src, m)) = chosen.iter().find(|(_, m)| {
        m.get("show")
            .or(m.get("title"))
            .is_some_and(|t| norm(t) != norm(&q.title))
            && !is_hit(m)
    }) {
        return Decision::Unresolved(
            "needs_flags",
            format!("{src}'s details are for {}, not \"{}\"", label(m), q.title),
        );
    }
    Decision::Sure
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d0(q: &Query, s: &Settings, f: &[(String, Meta)]) -> Decision {
        decide(q, s, f, 0)
    }
    fn m(src: &str, t: &str, y: &str) -> (String, Meta) {
        (
            src.into(),
            [
                ("title".to_string(), t.to_string()),
                ("year".to_string(), y.to_string()),
            ]
            .into(),
        )
    }
    fn q(t: &str, y: Option<&str>) -> Query {
        Query {
            title: t.into(),
            year: y.map(String::from),
            ..Default::default()
        }
    }
    #[test]
    fn acts_only_on_sure_information() {
        let none = Settings::default();
        assert_eq!(
            d0(
                &q("The Matrix", Some("1999")),
                &none,
                &[m("tmdb", "The Matrix", "1999")]
            ),
            Decision::Sure
        );
        assert!(matches!(
            d0(
                &q("The Matrix", Some("1999")),
                &none,
                &[m("tmdb", "The Matrix Reloaded", "2003")]
            ),
            Decision::Unresolved("no_match", _)
        ));
        assert_eq!(
            d0(
                &q("breaking.bad", None),
                &none,
                &[m("tmdb", "Breaking Bad", "2008")]
            ),
            Decision::Sure
        );
        assert!(matches!(
            d0(
                &q("Breaking Bad", None),
                &none,
                &[m("tmdb", "Breaking Bad: Original Minisodes", "2009")]
            ),
            Decision::Unresolved("no_match", _)
        ));
        let named = Settings {
            name: Some("Breaking Bad".into()),
            year: Some("2008".into()),
            ..Default::default()
        };
        assert_eq!(
            d0(&q("Breaking Bad", Some("2008")), &named, &[]),
            Decision::Sure
        );
        assert_eq!(
            d0(
                &Query {
                    imdb: Some("tt1".into()),
                    ..q("x", None)
                },
                &none,
                &[]
            ),
            Decision::Sure
        );
    }
    #[test]
    fn whole_lists_decide_uniqueness() {
        let none = Settings::default();
        // one search list with two exact names: ambiguous, even if the top one was fetched
        let f = [
            m("tmdb", "The Office", "2005"),
            m("tmdb:search", "The Office", "2005"),
            m("tmdb:search", "The Office", "2001"),
            m("tmdb:search", "The Office Mixup", "2020"),
        ];
        let d = d0(&q("The Office", None), &none, &f);
        assert!(
            matches!(&d, Decision::Unresolved("ambiguous", x) if x.contains("2001") && x.contains("2005")),
            "{d:?}"
        );
        // with the year it is unique
        assert_eq!(
            d0(&q("The Office", Some("2005")), &none, &f),
            Decision::Sure
        );
        // each list has one, but the sources disagree
        let f = [
            m("tmdb:search", "Dune", "2021"),
            m("thetvdb:search", "Dune", "1984"),
        ];
        assert!(matches!(
            d0(&q("Dune", None), &none, &f),
            Decision::Unresolved("needs_flags", _)
        ));
        // exactly one exact match across all lists, sources agree
        let f = [
            m("tmdb", "The Why Files", "2021"),
            m("tmdb:search", "The Why Files", "2021"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("thetvdb:search", "The Why Files", "2021"),
        ];
        assert_eq!(d0(&q("The Why Files", None), &none, &f), Decision::Sure);
        // the exact match was listed but the details fetched are another work's
        let f = [
            m("tmdb", "Why Women Kill", "2019"),
            m("tmdb:search", "Why Women Kill", "2019"),
            m("tmdb:search", "The Why Files", "2021"),
        ];
        assert!(matches!(
            d0(&q("The Why Files", None), &none, &f),
            Decision::Unresolved("needs_flags", _)
        ));
    }
    #[test]
    fn year_tolerance_is_a_hint_for_agreement() {
        let none = Settings::default();
        let f = [
            m("tmdb:search", "Amelie", "2001"),
            m("tvmaze:search", "Amelie", "2002"),
        ];
        assert_eq!(decide(&q("Amelie", None), &none, &f, 1), Decision::Sure);
        assert!(matches!(
            decide(&q("Amelie", None), &none, &f, 0),
            Decision::Unresolved("needs_flags", _)
        ));
    }
}
