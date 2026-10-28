//! A query that also carries the ids earlier providers found (a person reuses what they already learned).
use crate::meta::Meta;
use crate::providers::Query;

pub fn with_found_ids(q: &Query, found: &[(String, Meta)]) -> Query {
    let first = |k: &str| found.iter().find_map(|(_, m)| m.get(k).cloned());
    let mut q2 = q.clone();
    q2.imdb = q.imdb.clone().or(first("imdb_id"));
    q2.tmdb = q
        .tmdb
        .clone()
        .or(first("tmdb_series_id").map(|x| format!("/tv/{x}")))
        .or(first("tmdb_id"));
    q2.thetvdb = q
        .thetvdb
        .clone()
        .or(first("thetvdb_series_id"))
        .or(first("thetvdb_id"));
    q2
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_missing_ids_only() {
        let m: Meta = [("imdb_id", "tt1"), ("tmdb_series_id", "42")]
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .into();
        let q = Query {
            thetvdb: Some("7".into()),
            ..Default::default()
        };
        let q2 = with_found_ids(&q, &[("tmdb".into(), m)]);
        assert_eq!(
            (
                q2.imdb.as_deref(),
                q2.tmdb.as_deref(),
                q2.thetvdb.as_deref()
            ),
            (Some("tt1"), Some("/tv/42"), Some("7"))
        );
    }
}
