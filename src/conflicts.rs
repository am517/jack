//! Where sources about the same work disagree. Found by logic, recorded, never settled by assumption.
use crate::meta::Meta;

/// Compared fields; "year" is compared by its first four digits.
const FIELDS: [&str; 6] = ["title", "year", "date", "season", "episode", "episode_title"];

/// (field, [(source, value)]) where at least two sources differ (ignoring case/spacing/separators).
/// Only sources about the chosen work count: same title as the chosen one, or the file itself.
pub fn find(chosen: &Meta, found: &[(String, Meta)]) -> Vec<(String, Vec<(String, String)>)> {
    let name = chosen.get("show").or(chosen.get("title")).map_or("", |s| s);
    let same: Vec<&(String, Meta)> = found.iter().filter(|(s, m)| s == "embedded" || crate::resolver::exact(name, None, m))
        .filter(|(_, m)| m.get("kind").is_none_or(|k| Some(k) == chosen.get("kind"))).collect();
    let mut out = vec![];
    for f in FIELDS {
        if chosen.get("kind").is_some_and(|k| k == "episode") && (f == "title" || f == "year") {
            continue; // an episode's title/year mix show and episode values across sources
        }
        let vals: Vec<(String, String)> = same.iter().filter_map(|(s, m)| m.get(f).map(|v| (s.clone(), if f == "year" { v.chars().take(4).collect() } else { v.clone() }))).collect();
        let mut d: Vec<String> = vals.iter().map(|(_, v)| crate::resolver::norm(v)).collect();
        d.sort();
        d.dedup();
        if d.len() > 1 {
            out.push((f.to_string(), vals));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(kv: &[(&str, &str)]) -> Meta {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    #[test]
    fn records_disagreement_ignores_other_works() {
        let chosen = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]);
        let found = vec![
            ("tmdb".into(), m(&[("title", "The Matrix"), ("year", "1999-03-31")])),
            ("omdb".into(), m(&[("title", "the.matrix"), ("year", "1999")])),
            ("itunes".into(), m(&[("title", "The Matrix Reloaded"), ("year", "2003")])),
            ("embedded".into(), m(&[("title", "Matrix"), ("year", "1998")])),
        ];
        let c = find(&chosen, &found);
        assert_eq!(c.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["title", "year"]);
        assert!(!c[1].1.iter().any(|(s, _)| s == "itunes"));
        assert!(find(&chosen, &found[..2]).is_empty());
    }
}
