//! Handling one media file: fetch, art, tag, sort.
use crate::config::{Config, Settings, suffixed};
use crate::meta::{self, Meta};
use crate::providers::{self, Http};
use crate::{askart, sorter, tagger};
use std::fmt::Write as _;
use std::path::Path;
use crate::{scan::*, fetch::*, art::*, print::*};

pub(crate) fn process(path: &Path, s: &Settings, cfg: &Config, http: &dyn Http, out: &mut String, pr: &mut Printed) -> Result<(), String> {
    let dry = Settings::flag(s.dry_run);
    let (print, dl) = (Settings::flag(s.print), Settings::flag(s.download_only));
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
    let (mut meta, found) = fetch(&query(path, s, cfg)?, cfg, http, out)?;
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
    if s.youtube.is_some() {
        let _ = writeln!(out, "    note: --youtube is not supported yet (later phase)");
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
        if !dry {
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
