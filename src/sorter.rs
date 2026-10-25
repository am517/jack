//! Composite merge and template-based sort/rename.
use crate::meta::Meta;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct MergeCfg {
    pub default: Vec<String>,
    pub fields: BTreeMap<String, Vec<String>>,
}

/// For each field, the first provider (per its priority list, then any) with a value wins.
pub fn merge(results: &[(String, Meta)], cfg: &MergeCfg) -> Meta {
    let mut out = Meta::new();
    let keys: std::collections::BTreeSet<&String> = results.iter().flat_map(|(_, m)| m.keys()).collect();
    for k in keys {
        let order = cfg.fields.get(k).unwrap_or(&cfg.default);
        let by_prio = order.iter().find_map(|p| results.iter().find(|(n, _)| n == p).and_then(|(_, m)| m.get(k)));
        if let Some(v) = by_prio.or_else(|| results.iter().find_map(|(_, m)| m.get(k))) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct SortCfg {
    pub movie_template: String,
    pub tv_template: String,
    pub mode: String,
    pub illegal_chars: String,
    pub replacement: String,
}

/// Fill a template; values are sanitized so they can't add path components.
pub fn render(tpl: &str, m: &Meta, ext: &str, cfg: &SortCfg) -> String {
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(i) = rest.find('{') {
        out += &rest[..i];
        let Some(j) = rest[i..].find('}') else { break };
        let (key, fmt) = rest[i + 1..i + j].split_once(':').unwrap_or((&rest[i + 1..i + j], ""));
        let mut v = if key == "ext" { ext.to_string() } else { m.get(key).cloned().unwrap_or_default() };
        if let (Ok(w), Ok(n)) = (fmt.parse::<usize>(), v.parse::<u64>()) {
            v = format!("{n:0w$}");
        }
        out.extend(v.chars().map(|c| if cfg.illegal_chars.contains(c) { cfg.replacement.clone() } else { c.to_string() }));
        rest = &rest[i + j + 1..];
    }
    out += rest;
    // drop separators left by empty placeholders ("Show - S01E01 - .mkv")
    for (a, b) in [(" - .", "."), (" - /", "/"), (" -  - ", " - ")] {
        out = out.replace(a, b);
    }
    out
}

/// [naming]: which language names folders and files ("original" or a language code).
#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct NamingCfg {
    pub file_language: String,
    pub folder_language: String,
    /// Max bytes per path component (UTF-8 safe; the extension is kept).
    pub max_component_bytes: usize,
}

/// Names for `lang`: "original" swaps in original_* names where known; anything else (the primary
/// language, or a code not fetched) uses the primary names. Missing names fall back to the other.
fn localized(m: &Meta, lang: &str) -> Meta {
    let mut x = m.clone();
    if lang == "original" {
        for (k, o) in [("title", "original_title"), ("show", "original_show"), ("episode_title", "original_episode_title")] {
            if let Some(v) = m.get(o) {
                x.insert(k.into(), v.clone());
            }
        }
    }
    x
}

/// NFC, control chars -> replacement, no trailing dots/spaces, at most `max` bytes (cut at a char boundary,
/// keeping `.ext` when `ext` is given).
pub fn safe_component(c: &str, ext: Option<&str>, cfg: &SortCfg, max: usize) -> String {
    use unicode_normalization::UnicodeNormalization;
    let c: String = c.nfc().map(|ch| if ch.is_control() { cfg.replacement.clone() } else { ch.to_string() }).collect();
    let suffix = ext.filter(|e| !e.is_empty()).map(|e| format!(".{e}")).filter(|s| c.ends_with(s.as_str()));
    let mut stem = c[..c.len() - suffix.as_ref().map_or(0, |s| s.len())].to_string();
    let budget = if max == 0 { usize::MAX } else { max.saturating_sub(suffix.as_ref().map_or(0, |s| s.len())) };
    if stem.len() > budget {
        let mut i = budget;
        while !stem.is_char_boundary(i) {
            i -= 1;
        }
        stem.truncate(i);
    }
    let stem = stem.trim_end_matches(['.', ' ']);
    let out = format!("{stem}{}", suffix.unwrap_or_default());
    if out.is_empty() { cfg.replacement.clone() } else { out }
}

pub fn destination(root: &Path, file: &Path, m: &Meta, cfg: &SortCfg, n: &NamingCfg) -> PathBuf {
    let ext = file.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let tv = m.get("kind").is_some_and(|k| k == "episode");
    let tpl = if tv { &cfg.tv_template } else { &cfg.movie_template };
    let folders = render(tpl, &localized(m, &n.folder_language), &ext, cfg);
    let name = render(tpl, &localized(m, &n.file_language), &ext, cfg);
    let mut parts: Vec<&str> = folders.split('/').collect();
    parts.pop();
    let mut p = root.to_path_buf();
    for c in parts.iter().filter(|c| !c.is_empty()) {
        p.push(safe_component(c, None, cfg, n.max_component_bytes));
    }
    p.push(safe_component(name.rsplit('/').next().unwrap_or(""), Some(&ext), cfg, n.max_component_bytes));
    p
}

pub fn apply(from: &Path, to: &Path, cfg: &SortCfg) -> Result<(), String> {
    let e = |x: std::io::Error| format!("{} -> {}: {x}", from.display(), to.display());
    if to.exists() {
        return Err(format!("{}: already exists", to.display()));
    }
    std::fs::create_dir_all(to.parent().unwrap_or(Path::new("."))).map_err(e)?;
    match cfg.mode.as_str() {
        "copy" => std::fs::copy(from, to).map(|_| ()).map_err(e),
        "move" => std::fs::rename(from, to).or_else(|_| std::fs::copy(from, to).and_then(|_| std::fs::remove_file(from))).map_err(e),
        x => Err(format!("[sort] mode: unknown '{x}' (use move or copy)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &[(&str, &str)]) -> Meta {
        p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn merge_uses_field_priority() {
        let cfg = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap().merge;
        let r = vec![
            ("tmdb".to_string(), m(&[("episode_title", "A"), ("description", "tm"), ("genre", "Comedy")])),
            ("thetvdb".to_string(), m(&[("episode_title", "B"), ("description", "tv"), ("year", "2016")])),
        ];
        let x = merge(&r, &cfg);
        assert_eq!(x["episode_title"], "B"); // thetvdb first for episode titles
        assert_eq!(x["description"], "tm");
        assert_eq!((x["genre"].as_str(), x["year"].as_str()), ("Comedy", "2016"));
    }

    #[test]
    fn templates_and_move() {
        let all = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let (cfg, en) = (all.sort, NamingCfg { file_language: "en".into(), ..all.naming });
        let ep = m(&[("kind", "episode"), ("show", "Saiki K"), ("season", "1"), ("episode", "2"), ("episode_title", "Who/What?")]);
        let d = destination(Path::new("/out"), Path::new("x.mkv"), &ep, &cfg, &en);
        assert_eq!(d, PathBuf::from("/out/TV/Saiki K/Season 01/Saiki K - S01E02 - Who-What-.mkv"));
        let mv = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]);
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("a.mp4");
        std::fs::write(&src, "x").unwrap();
        let dst = destination(t.path(), &src, &mv, &cfg, &en);
        assert!(dst.ends_with("Movies/The Matrix (1999)/The Matrix (1999).mp4"));
        apply(&src, &dst, &cfg).unwrap();
        assert!(dst.exists() && !src.exists());
        std::fs::write(&src, "y").unwrap();
        assert!(apply(&src, &dst, &cfg).unwrap_err().contains("already exists"));
    }

    #[test]
    fn bilingual_naming_fallback_and_safety() {
        let all = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let (cfg, n) = (all.sort, all.naming); // defaults: files original, folders en
        let ep = m(&[("kind", "episode"), ("show", "The Apothecary Diaries"), ("original_show", "薬屋のひとりごと"), ("season", "3"), ("episode", "1"), ("episode_title", "Locusts"), ("original_episode_title", "蝗")]);
        let d = destination(Path::new("/out"), Path::new("a.mp4"), &ep, &cfg, &n);
        assert_eq!(d, PathBuf::from("/out/TV/The Apothecary Diaries/Season 03/薬屋のひとりごと - S03E01 - 蝗.mp4"));
        // original missing -> primary; primary-only folder lang with original file lang swapped
        let mut half = ep.clone();
        half.remove("original_episode_title");
        let d = destination(Path::new("/o"), Path::new("a.mp4"), &half, &cfg, &n);
        assert!(d.ends_with("薬屋のひとりごと - S03E01 - Locusts.mp4"), "{d:?}");
        let sw = NamingCfg { file_language: "en".into(), folder_language: "original".into(), ..n.clone() };
        let d = destination(Path::new("/o"), Path::new("a.mp4"), &ep, &cfg, &sw);
        assert_eq!(d, PathBuf::from("/o/TV/薬屋のひとりごと/Season 03/The Apothecary Diaries - S03E01 - Locusts.mp4"));
        let mut no_orig = ep.clone();
        no_orig.retain(|k, _| !k.starts_with("original"));
        assert!(destination(Path::new("/o"), Path::new("a.mp4"), &no_orig, &cfg, &sw).starts_with("/o/TV/The Apothecary Diaries"));
        // NFC: decomposed e + combining acute -> é
        assert_eq!(safe_component("Poke\u{301}mon\u{1}x. ", None, &cfg, 255), "Pokémon-x");
        let mv = m(&[("kind", "movie"), ("title", "Amélie: A/B? \"x\" <y>|z\\"), ("year", "2001")]);
        let d = destination(Path::new("/o"), Path::new("a.mkv"), &mv, &cfg, &n);
        assert_eq!(d, PathBuf::from("/o/Movies/Amélie- A-B- -x- -y--z- (2001)/Amélie- A-B- -x- -y--z- (2001).mkv"));
        // byte cap at a char boundary, extension kept
        let s = safe_component(&format!("{}.mp4", "薬".repeat(100)), Some("mp4"), &cfg, 20);
        assert_eq!(s, format!("{}.mp4", "薬".repeat(5)));
        assert!(s.len() <= 20);
    }
}
