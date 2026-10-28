//! Querying providers and merging their answers.
use crate::config::Config;
use crate::meta::Meta;
use crate::providers::{self, Http, Omdb, Provider, Query, Tmdb, Tvdb};
use crate::{askart, sorter};
use std::fmt::Write as _;

/// Composite metadata from all providers in [providers] order, merged per [merge].
pub(crate) fn provider_cfgs(cfg: &Config) -> (providers::TmdbCfg, providers::TvdbCfg) {
    let pc = &cfg.providers;
    let mut tc = pc.tmdb.clone();
    tc.keep_original = pc.keep_original;
    if tc.language.is_empty() {
        tc.language = pc.language.clone();
    }
    let mut vc = pc.thetvdb.clone();
    vc.keep_original = pc.keep_original;
    if vc.language.is_empty() {
        vc.language = pc.language_codes.get(&pc.language).cloned().unwrap_or_else(|| pc.language.clone());
    }
    (tc, vc)
}

/// --ask-art only: full image lists from TMDB and TheTVDB for the providers that matched.
pub(crate) fn all_images(found: &[(String, Meta)], cfg: &Config, http: &dyn Http) -> Vec<askart::Cand> {
    let (tc, vc) = provider_cfgs(cfg);
    let mut v = vec![];
    for (p, m) in found {
        match p.as_str() {
            "tmdb" => v.extend(Tmdb { cfg: tc.clone(), http }.images(m)),
            "thetvdb" => v.extend(Tvdb::new(vc.clone(), http).images(m)),
            _ => {}
        }
    }
    v
}

pub(crate) fn fetch(q: &Query, cfg: &Config, http: &dyn Http, out: &mut String) -> Result<(Meta, Vec<(String, Meta)>), String> {
    let pc = &cfg.providers;
    let (tc, vc) = provider_cfgs(cfg);
    let tmdb = Tmdb { cfg: tc, http };
    let tvdb = Tvdb::new(vc, http);
    let omdb = Omdb { cfg: cfg.providers.omdb.clone(), http };
    let (mut found, mut errs) = (vec![], vec![]);
    for name in &cfg.providers.order {
        let p: &dyn Provider = match name.as_str() {
            "tmdb" => &tmdb,
            "thetvdb" => &tvdb,
            "omdb" => &omdb,
            x => return Err(format!("[providers] order: unknown provider '{x}'")),
        };
        match p.lookup(q) {
            Ok(Some(mut m)) => {
                if let Some(w) = m.remove(providers::WARNING) {
                    errs.push(format!("{}: {w}", p.name()));
                }
                found.push((name.clone(), m))
            }
            Ok(None) => {}
            Err(e) => errs.push(format!("{}: {e}", p.name())),
        }
    }
    if found.is_empty() {
        return Err(if errs.is_empty() { format!("no match for '{}'", q.title) } else { errs.join("; ") });
    }
    for e in errs {
        let _ = writeln!(out, "    warning: {e}");
    }
    let mut m = sorter::merge(&found, &cfg.merge);
    match providers::merge_genres(found.iter().filter_map(|(_, x)| x.get("genre").map(String::as_str)), &pc.genre_case, &pc.genre_separator) {
        Some(g) => m.insert("genre".into(), g),
        None => m.remove("genre"),
    };
    let ep = m.get("kind").is_some_and(|k| k == "episode");
    if ep && !m.contains_key("episode_title") && cfg.matching.episode_title_from_filename {
        if let Some(t) = &q.episode_title {
            let _ = writeln!(out, "    warning: no episode title from providers; using the filename's ({t})");
            m.insert("episode_title".into(), t.clone());
        }
    }
    if ep && pc.tv_show_year {
        if let (Some(s), Some(y)) = (m.get("show").cloned(), m.get("year").cloned()) {
            m.insert("show".into(), format!("{s} ({y})"));
        }
    }
    if pc.original_in_description {
        if let (Some(o), Some(d)) = (m.get("original_title").cloned(), m.get("description").cloned()) {
            m.insert("description".into(), format!("{d}\n\nOriginal title: {o}"));
        }
    }
    Ok((m, found))
}
