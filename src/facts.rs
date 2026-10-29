//! The file's own metadata is the truth for facts, not for wording. Jack may keep a source's
//! refined/expanded value when it is clearly the same thing ("Joe Rogan" -> "The Joe Rogan
//! Experience"); a contradicting value ("Monica Rogan", another year) never replaces the file's
//! fact: the file's value stays and the disagreement is recorded as a conflict (see conflicts).
//! Original values are always stored (the "embedded" source row).
use crate::meta::Meta;
use crate::resolver::norm;

/// Fields where the file's value is a fact.
pub const FACTS: [&str; 7] = ["title", "show", "year", "date", "season", "episode", "episode_title"];
/// Exact facts: any difference is a contradiction.
const EXACT: [&str; 4] = ["year", "date", "season", "episode"];

/// `long` says the same as `short`, only more fully: all of short's words, in order, inside long.
/// "Joe Rogan" -> "The Joe Rogan Experience" yes; "Joe Rogan" -> "Monica Rogan" no.
pub fn refines(field: &str, short: &str, long: &str) -> bool {
    let (s, l) = (norm(short), norm(long));
    if s == l {
        return true;
    }
    if EXACT.contains(&field) || s.is_empty() {
        return false;
    }
    let lw: Vec<&str> = l.split(' ').collect();
    let mut i = 0;
    for w in s.split(' ') {
        match lw[i..].iter().position(|x| *x == w) {
            Some(p) => i += p + 1,
            None => return false,
        }
    }
    true
}

/// Puts the file's facts back where a source contradicts them. Returns the fields kept from the file.
pub fn apply(meta: &mut Meta, own: &Meta) -> Vec<String> {
    let mut kept = vec![];
    for f in FACTS {
        let Some(o) = own.get(f).filter(|v| !v.trim().is_empty()) else { continue };
        let o = if f == "year" { o.chars().take(4).collect() } else { o.clone() };
        match meta.get(f) {
            Some(v) if refines(f, &o, v) => {}
            Some(_) => {
                meta.insert(f.into(), o);
                kept.push(f.to_string());
            }
            None => {
                meta.insert(f.into(), o);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(kv: &[(&str, &str)]) -> Meta {
        kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
    #[test]
    fn refine_yes_contradict_no() {
        assert!(refines("show", "Joe Rogan", "The Joe Rogan Experience"));
        assert!(refines("title", "breaking.bad", "Breaking Bad"));
        assert!(!refines("show", "Joe Rogan", "Monica Rogan"));
        assert!(!refines("show", "Rogan Joe", "The Joe Rogan Experience"));
        assert!(!refines("year", "199", "1999"));
        let mut meta = m(&[("show", "The Joe Rogan Experience"), ("year", "2010")]);
        assert!(apply(&mut meta, &m(&[("show", "Joe Rogan"), ("year", "2010")])).is_empty());
        assert_eq!(meta["show"], "The Joe Rogan Experience");
        let mut meta = m(&[("show", "Monica Rogan"), ("year", "2011")]);
        assert_eq!(apply(&mut meta, &m(&[("show", "Joe Rogan"), ("year", "2010-05-01"), ("season", "2")])), ["show", "year"]);
        assert_eq!((meta["show"].as_str(), meta["year"].as_str(), meta["season"].as_str()), ("Joe Rogan", "2010", "2"));
    }
}
