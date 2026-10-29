//! Handling one media file: fetch, art, tag, sort.
use crate::config::{Config, Settings, suffixed};
use crate::meta::{self, Meta};
use crate::providers::{self, Http};
use crate::{askart, online, sorter, store, tagger};
use std::fmt::Write as _;
use std::path::Path;
use crate::{scan::*, fetch::*, art::*, print::*};

pub(crate) fn process(path: &Path, s: &Settings, cfg: &Config, http: &dyn Http, out: &mut String, pr: &mut Printed) -> Result<(), String> {
    let dry = Settings::flag(s.dry_run);
    let (print, dl) = (Settings::flag(s.print), Settings::flag(s.download_only));
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
    let (mut meta, found, unresolved) = match online_label(path, s, cfg) {
        Some(l) => {
            let o = online::fetch(&l, &show_flags(s), cfg, http, &crate::resolver::Strict, out)?;
            (o.meta, o.found, o.unresolved)
        }
        None => {
            let q = query(path, s, cfg)?;
            let (mut m, f) = fetch(&q, cfg, http, out)?;
            let sure = sure(&q, s, &f);
            if !sure {
                let _ = writeln!(out, "    not possible with the available information: \"{}\"{} (give --name and --year, or an id)", q.title, q.year.as_deref().map(|y| format!(" ({y})")).unwrap_or_default());
                m.insert("resolution".into(), "unresolved".into());
            }
            (m, f, !sure)
        }
    };
    let t = &cfg.tagger;
    // art is kept beside the file only with -d; otherwise it's a temp file removed after tagging
    let pp = art_path(path, s, dl);
    if !dry && t.ask_art && (dl || (t.embed_artwork && !print)) {
        let prefs = |k: &str| cfg.merge.fields.get(k).unwrap_or(&cfg.merge.default).clone();
        let c = askart::candidates(&found, all_images(&found, cfg, http), &art_keys(&meta, s, cfg), prefs, t.ask_art_max);
        let open = (t.ask_art_open_folder.unwrap_or(cfg!(target_os = "macos")) && !t.ask_art_open_command.is_empty()).then_some(t.ask_art_open_command.as_str());
        let o = askart::Opts { open_cmd: open, keep: dl || t.ask_art_keep };
        match askart::ask(path, &pp, &c, http, &o, out) {
            Ok(Some(p)) => {
                meta.insert("poster_file".into(), p.to_string_lossy().into_owned());
            }
            Ok(None) => {}
            Err(e) => {
                let _ = writeln!(out, "    warning: artwork: {e}");
            }
        }
    } else if !dry && (dl || (t.embed_artwork && !print)) {
        // art is optional: never lose the metadata over it
        if let Err(e) = save_poster(&pp, s, cfg, &mut meta, http) {
            let _ = writeln!(out, "    warning: artwork: {e}");
        }
    }
    if let Some(g) = &s.grouping {
        meta.insert("grouping".into(), g.clone());
    }
    let _ = writeln!(out, "    ids: {}", found_ids(&meta, s));
    if !dry && cfg.store.enabled && (dl || !print) {
        match store::save(path, &meta, &found, &cfg.store) {
            Ok(w) => {
                let _ = writeln!(out, "    {w}");
            }
            Err(e) => {
                let _ = writeln!(out, "    warning: store: {e}");
            }
        }
    }
    if unresolved {
        // stored with every candidate above; a wrong tag is worse than none
        let _ = writeln!(out, "    unresolved: stored with all candidates; not tagged or sorted");
        if print {
            print_meta(path, s, cfg, &meta, pr);
        }
        return Ok(());
    }
    if print {
        print_meta(path, s, cfg, &meta, pr);
        if !dl {
            return Ok(());
        }
    }
    if dl {
        let mp = suffixed(path, msuf);
        if !dry {
            meta::save(&mp, &meta)?;
        }
        let _ = writeln!(out, "    save metadata: {}", mp.display());
        return Ok(());
    }
    if let Some(i) = s.imdb.as_deref().and_then(providers::imdb_id) {
        meta.entry("imdb_id".into()).or_insert(i);
    }
    let tagged = tag(path, &meta, cfg, dry, out);
    if let Some(p) = meta.get("poster_file") {
        let _ = std::fs::remove_file(p);
    }
    tagged?;
    if s.output.is_some() || cfg.sort.enabled {
        let to = sorter::destination(s.output.as_deref(), path, &meta, &cfg.sort, &cfg.naming);
        let _ = writeln!(out, "    {}: {}", cfg.sort.mode, to.display());
        if !dry {
            sorter::apply(path, &to, &cfg.sort)?;
        }
    }
    Ok(())
}

pub(crate) fn tag(path: &Path, meta: &Meta, cfg: &Config, dry: bool, out: &mut String) -> Result<(), String> {
    let cmd = tagger::command(path, meta, &cfg.tagger)?;
    for c in std::iter::once(cmd).chain(tagger::extra_commands(path, meta, &cfg.tagger)) {
        let _ = writeln!(out, "    tag: {}", tagger::show(&c));
        if dry {
            tagger::discard(&c);
        } else {
            tagger::run(&c)?;
        }
    }
    Ok(())
}

/// Ids in the metadata (else the requested ones).
pub(crate) fn found_ids(m: &Meta, s: &Settings) -> String {
    let v: Vec<String> = ["imdb_id", "tmdb_id", "tmdb_episode_id", "thetvdb_id", "thetvdb_episode_id"].iter().filter_map(|k| m.get(*k).map(|v| format!("{}={v}", k.trim_end_matches("_id")))).collect();
    if v.is_empty() { ids(s) } else { v.join(" ") }
}

pub(crate) fn ids(s: &Settings) -> String {
    let v: Vec<String> = [("imdb", &s.imdb), ("tmdb", &s.tmdb), ("thetvdb", &s.thetvdb), ("youtube", &s.youtube), ("name", &s.name), ("year", &s.year)]
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
        .collect();
    if v.is_empty() { "-".into() } else { v.join(" ") }
}

/// Online label: flags first, then what the file itself says.
fn online_label(path: &Path, s: &Settings, cfg: &Config) -> Option<online::Label> {
    online::from_flags(s.youtube.as_deref(), s.x.as_deref()).or_else(|| cfg.online.enabled.then(|| online::detect(path, &cfg.online)).flatten())
}

/// Show ids given as flags alongside an online video.
fn show_flags(s: &Settings) -> Meta {
    let mut m = Meta::new();
    for (k, v) in [("tmdb_series_id", &s.tmdb), ("thetvdb_series_id", &s.thetvdb), ("imdb_id", &s.imdb)] {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
            m.insert(k.into(), v.trim_start_matches("/tv/").into());
        }
    }
    m
}

/// Enough to act on: an id, --name with --year, or a provider answer whose title and year match exactly.
pub(crate) fn sure(q: &crate::providers::Query, s: &Settings, found: &[(String, Meta)]) -> bool {
    let set = |o: &Option<String>| o.as_ref().is_some_and(|v| !v.is_empty());
    if q.imdb.is_some() || q.tmdb.is_some() || q.thetvdb.is_some() || (set(&s.name) && set(&s.year)) {
        return true;
    }
    let Some(y) = q.year.as_deref() else { return false };
    let year_of = |m: &Meta| m.get("series_year").or(m.get("year")).cloned();
    found.iter().any(|(_, m)| {
        let mut m = m.clone();
        if let Some(y) = year_of(&m) {
            m.insert("year".into(), y);
        }
        crate::resolver::exact(&q.title, Some(y), &m)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::Query;
    fn m(t: &str, y: &str) -> Vec<(String, Meta)> {
        vec![("tmdb".into(), [("title".to_string(), t.to_string()), ("year".to_string(), y.to_string())].into())]
    }
    #[test]
    fn acts_only_on_sure_information() {
        let q = |t: &str, y: Option<&str>| Query { title: t.into(), year: y.map(String::from), ..Default::default() };
        let none = Settings::default();
        assert!(sure(&q("The Matrix", Some("1999")), &none, &m("The Matrix", "1999")));
        assert!(!sure(&q("The Matrix", Some("1999")), &none, &m("The Matrix Reloaded", "2003")));
        assert!(!sure(&q("Breaking Bad", None), &none, &m("Breaking Bad", "2008")));
        let named = Settings { name: Some("Breaking Bad".into()), year: Some("2008".into()), ..Default::default() };
        assert!(sure(&q("Breaking Bad", Some("2008")), &named, &[]));
        assert!(sure(&Query { imdb: Some("tt1".into()), ..q("x", None) }, &none, &[]));
    }
}
