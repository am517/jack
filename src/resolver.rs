//! Deciding among candidates. Jack never best-guesses: the default `Strict` resolver picks only
//! an exact, unique match. Sammy (a future LLM windup) can implement `Resolver` and be passed in.
use crate::meta::Meta;

/// (source, metadata)
pub type Candidate = (String, Meta);

#[derive(Debug, PartialEq)]
pub enum Choice {
    Pick(usize),
    /// No candidate matches exactly.
    None,
    /// More than one matches exactly: not possible with the available information.
    Unresolved,
}

pub trait Resolver {
    /// `title` and `year` (if known) are what the label says; candidates carry "title"/"show" and "year".
    fn resolve(&self, title: &str, year: Option<&str>, cands: &[Candidate]) -> Choice;
}

/// Exactly one candidate whose title (and year, when given) match exactly.
pub struct Strict;

impl Resolver for Strict {
    fn resolve(&self, title: &str, year: Option<&str>, c: &[Candidate]) -> Choice {
        let hits: Vec<usize> = (0..c.len()).filter(|&i| exact(title, year, &c[i].1)).collect();
        match hits.as_slice() {
            [] => Choice::None,
            [i] => Choice::Pick(*i),
            _ => Choice::Unresolved,
        }
    }
}

/// Same title (ignoring only case and spacing) and, if a year is given, same year.
pub fn exact(title: &str, year: Option<&str>, m: &Meta) -> bool {
    let t = m.get("show").or(m.get("title")).map_or("", |s| s);
    norm(t) == norm(title) && !norm(title).is_empty() && year.is_none_or(|y| m.get("year").is_some_and(|my| my == y))
}

fn norm(s: &str) -> String {
    s.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(t: &str, y: &str) -> Candidate {
        ("tmdb".into(), [("title".to_string(), t.to_string()), ("year".to_string(), y.to_string())].into())
    }
    #[test]
    fn strict_only_exact_unique() {
        let s = Strict;
        assert_eq!(s.resolve("The Why Files", None, &[c("Why Women Kill", "2019"), c("The Why Files", "2021")]), Choice::Pick(1));
        assert_eq!(s.resolve("The Office", None, &[c("The Office", "2005"), c("The Office", "2001")]), Choice::Unresolved);
        assert_eq!(s.resolve("The Office", Some("2005"), &[c("The Office", "2005"), c("The Office", "2001")]), Choice::Pick(0));
        // close is not enough
        assert_eq!(s.resolve("The Why Files", None, &[c("The Why Files?", "2005")]), Choice::None);
        assert_eq!(s.resolve("the  why files", None, &[c("The Why Files", "2021")]), Choice::Pick(0));
        assert_eq!(s.resolve("Joe Rogan Experience", None, &[c("The Joe Rogan Experience", "2009")]), Choice::None);
        assert_eq!(s.resolve("Dr. Shawn Baker", None, &[]), Choice::None);
    }
}
