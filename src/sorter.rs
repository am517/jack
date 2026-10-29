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
    /// Fields where the longest value wins (Jack's fallback until Sammy picks).
    pub longest: Vec<String>,
}

/// For each field, the first provider (per its priority list, then any) with a value wins;
/// `longest` fields take the longest value (ties: priority order).
pub fn merge(results: &[(String, Meta)], cfg: &MergeCfg) -> Meta {
    let mut out = Meta::new();
    let keys: std::collections::BTreeSet<&String> =
        results.iter().flat_map(|(_, m)| m.keys()).collect();
    for k in keys {
        let order = cfg.fields.get(k).unwrap_or(&cfg.default);
        if cfg.longest.contains(k) {
            let ranked = order
                .iter()
                .filter_map(|p| results.iter().find(|(n, _)| n == p))
                .chain(results.iter().filter(|(n, _)| !order.contains(n)));
            if let Some(v) = ranked
                .filter_map(|(_, m)| m.get(k))
                .rev()
                .max_by_key(|v| v.chars().count())
            {
                out.insert(k.clone(), v.clone());
            }
            continue;
        }
        let by_prio = order.iter().find_map(|p| {
            results
                .iter()
                .find(|(n, _)| n == p)
                .and_then(|(_, m)| m.get(k))
        });
        if let Some(v) = by_prio.or_else(|| results.iter().find_map(|(_, m)| m.get(k))) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct SortCfg {
    /// Sort without -o (into tv_root / movie_root). -o always sorts.
    pub enabled: bool,
    pub tv_root: String,
    pub movie_root: String,
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
        let (key, fmt) = rest[i + 1..i + j]
            .split_once(':')
            .unwrap_or((&rest[i + 1..i + j], ""));
        let mut v = if key == "ext" {
            ext.to_string()
        } else {
            m.get(key).cloned().unwrap_or_default()
        };
        if let (Ok(w), Ok(n)) = (fmt.parse::<usize>(), v.parse::<u64>()) {
            v = format!("{n:0w$}");
        }
        out.extend(v.chars().map(|c| {
            if cfg.illegal_chars.contains(c) {
                cfg.replacement.clone()
            } else {
                c.to_string()
            }
        }));
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
    /// Longest file or folder name (extension kept); 0 = no limit.
    pub filename_max: usize,
    /// Unit of filename_max: "chars", "bytes", or "auto" (chars on macOS, bytes elsewhere).
    pub filename_unit: String,
}

impl NamingCfg {
    /// True when filename_max counts UTF-8 bytes.
    pub fn in_bytes(&self) -> bool {
        match self.filename_unit.as_str() {
            "chars" => false,
            "bytes" => true,
            _ => !cfg!(target_os = "macos"),
        }
    }
}

/// Names for `lang`: "original" swaps in original_* names where known; anything else (the primary
/// language, or a code not fetched) uses the primary names. Missing names fall back to the other.
fn localized(m: &Meta, lang: &str) -> Meta {
    let mut x = m.clone();
    if lang == "original" {
        for (k, o) in [
            ("title", "original_title"),
            ("show", "original_show"),
            ("episode_title", "original_episode_title"),
        ] {
            if let Some(v) = m.get(o) {
                x.insert(k.into(), v.clone());
            }
        }
    }
    x
}

/// NFC, control chars -> replacement, no trailing dots/spaces, at most `max` bytes (or characters when
/// `!bytes`; 0 = no limit), cut at a char boundary keeping `.ext` when `ext` is given.
pub fn safe_component(
    c: &str,
    ext: Option<&str>,
    cfg: &SortCfg,
    max: usize,
    bytes: bool,
) -> String {
    use unicode_normalization::UnicodeNormalization;
    let len = |s: &str| if bytes { s.len() } else { s.chars().count() };
    let c: String = c
        .nfc()
        .map(|ch| {
            if ch.is_control() {
                cfg.replacement.clone()
            } else {
                ch.to_string()
            }
        })
        .collect();
    let suffix = ext
        .filter(|e| !e.is_empty())
        .map(|e| format!(".{e}"))
        .filter(|s| c.ends_with(s.as_str()));
    let mut stem = c[..c.len() - suffix.as_ref().map_or(0, |s| s.len())].to_string();
    let budget = max.saturating_sub(suffix.as_deref().map_or(0, len));
    if max > 0 && len(&stem) > budget {
        let mut cut = String::new();
        for ch in stem.chars() {
            if len(&cut) + len(ch.encode_utf8(&mut [0; 4])) > budget {
                break;
            }
            cut.push(ch);
        }
        stem = cut.trim_end_matches([' ', '-', '.']).to_string();
    }
    let stem = stem.trim_end_matches(['.', ' ']);
    let out = format!("{stem}{}", suffix.unwrap_or_default());
    if out.is_empty() {
        cfg.replacement.clone()
    } else {
        out
    }
}

/// Target path. Root: `output` (-o) if given, else tv_root / movie_root (~ expanded). A leading
/// {tv_root}/{movie_root} in the template stands for that root; other templates are relative to it.
pub fn destination(
    output: Option<&Path>,
    file: &Path,
    m: &Meta,
    cfg: &SortCfg,
    n: &NamingCfg,
) -> PathBuf {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tv = m.get("kind").is_some_and(|k| k == "episode");
    let (tpl, var, def) = if tv {
        (&cfg.tv_template, "{tv_root}", &cfg.tv_root)
    } else {
        (&cfg.movie_template, "{movie_root}", &cfg.movie_root)
    };
    let tpl = tpl.strip_prefix(var).unwrap_or(tpl).trim_start_matches('/');
    let mut p = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate::config::expand(Path::new(def)));
    // {x_en}: primary-language name, {x_file} / {x_folder}: per file_language / folder_language
    let mut m = m.clone();
    let (base, fl, dl) = (
        m.clone(),
        localized(&m, &n.file_language),
        localized(&m, &n.folder_language),
    );
    for k in ["show", "title", "episode_title"] {
        for (suf, src) in [("en", &base), ("file", &fl), ("folder", &dl)] {
            if let Some(v) = src.get(k) {
                m.insert(format!("{k}_{suf}"), v.clone());
            }
        }
    }
    if let Some(y) = m.get("year").cloned() {
        m.entry("show_year".into()).or_insert(y);
    }
    let folders = render(tpl, &localized(&m, &n.folder_language), &ext, cfg);
    let name = render(tpl, &localized(&m, &n.file_language), &ext, cfg);
    let mut parts: Vec<&str> = folders.split('/').collect();
    parts.pop();
    for c in parts.iter().filter(|c| !c.is_empty()) {
        p.push(safe_component(c, None, cfg, n.filename_max, n.in_bytes()));
    }
    p.push(safe_component(
        name.rsplit('/').next().unwrap_or(""),
        Some(&ext),
        cfg,
        n.filename_max,
        n.in_bytes(),
    ));
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
        "move" => std::fs::rename(from, to)
            .or_else(|_| std::fs::copy(from, to).and_then(|_| std::fs::remove_file(from)))
            .map_err(e),
        x => Err(format!("[sort] mode: unknown '{x}' (use move or copy)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(p: &[(&str, &str)]) -> Meta {
        p.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn merge_uses_field_priority() {
        let cfg = crate::config::load(Some(Path::new("config/jack.toml")))
            .unwrap()
            .merge;
        let r = vec![
            (
                "tmdb".to_string(),
                m(&[
                    ("episode_title", "A"),
                    ("description", "tm"),
                    ("genre", "Comedy"),
                ]),
            ),
            (
                "thetvdb".to_string(),
                m(&[
                    ("episode_title", "B"),
                    ("description", "tv"),
                    ("year", "2016"),
                ]),
            ),
        ];
        let x = merge(&r, &cfg);
        assert_eq!(x["episode_title"], "B"); // thetvdb first for episode titles
        assert_eq!(x["description"], "tm"); // longest; tie -> priority
        let mut r2 = r.clone();
        r2[1]
            .1
            .insert("description".into(), "the longer one".into());
        assert_eq!(merge(&r2, &cfg)["description"], "the longer one");
        assert_eq!(
            merge(
                &r2,
                &MergeCfg {
                    longest: vec![],
                    ..cfg.clone()
                }
            )["description"],
            "tm"
        );
        assert_eq!(
            (x["genre"].as_str(), x["year"].as_str()),
            ("Comedy", "2016")
        );
    }

    #[test]
    fn templates_and_move() {
        let all = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let (cfg, en) = (
            all.sort,
            NamingCfg {
                file_language: "en".into(),
                ..all.naming
            },
        );
        let cfg = SortCfg { movie_template: "Movies/{title} ({year})/{title} ({year}).{ext}".into(), tv_template: "TV/{show}/Season {season:02}/{show} - S{season:02}E{episode:02} - {episode_title}.{ext}".into(), ..cfg }; // relative (legacy) templates
        let ep = m(&[
            ("kind", "episode"),
            ("show", "Saiki K"),
            ("season", "1"),
            ("episode", "2"),
            ("episode_title", "Who/What?"),
        ]);
        let d = destination(Some(Path::new("/out")), Path::new("x.mkv"), &ep, &cfg, &en);
        assert_eq!(
            d,
            PathBuf::from("/out/TV/Saiki K/Season 01/Saiki K - S01E02 - Who-What-.mkv")
        );
        let mv = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]);
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("a.mp4");
        std::fs::write(&src, "x").unwrap();
        let dst = destination(Some(t.path()), &src, &mv, &cfg, &en);
        assert!(dst.ends_with("Movies/The Matrix (1999)/The Matrix (1999).mp4"));
        apply(&src, &dst, &cfg).unwrap();
        assert!(dst.exists() && !src.exists());
        std::fs::write(&src, "y").unwrap();
        assert!(
            apply(&src, &dst, &cfg)
                .unwrap_err()
                .contains("already exists")
        );
    }

    #[test]
    fn bilingual_naming_fallback_and_safety() {
        let all = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let (cfg, n) = (all.sort, all.naming); // defaults: files original, folders en
        let cfg = SortCfg { movie_template: "Movies/{title} ({year})/{title} ({year}).{ext}".into(), tv_template: "TV/{show}/Season {season:02}/{show} - S{season:02}E{episode:02} - {episode_title}.{ext}".into(), ..cfg }; // relative (legacy) templates
        let ep = m(&[
            ("kind", "episode"),
            ("show", "The Apothecary Diaries"),
            ("original_show", "薬屋のひとりごと"),
            ("season", "3"),
            ("episode", "1"),
            ("episode_title", "Locusts"),
            ("original_episode_title", "蝗"),
        ]);
        let d = destination(Some(Path::new("/out")), Path::new("a.mp4"), &ep, &cfg, &n);
        assert_eq!(
            d,
            PathBuf::from(
                "/out/TV/The Apothecary Diaries/Season 03/薬屋のひとりごと - S03E01 - 蝗.mp4"
            )
        );
        // original missing -> primary; primary-only folder lang with original file lang swapped
        let mut half = ep.clone();
        half.remove("original_episode_title");
        let d = destination(Some(Path::new("/o")), Path::new("a.mp4"), &half, &cfg, &n);
        assert!(
            d.ends_with("薬屋のひとりごと - S03E01 - Locusts.mp4"),
            "{d:?}"
        );
        let sw = NamingCfg {
            file_language: "en".into(),
            folder_language: "original".into(),
            ..n.clone()
        };
        let d = destination(Some(Path::new("/o")), Path::new("a.mp4"), &ep, &cfg, &sw);
        assert_eq!(
            d,
            PathBuf::from(
                "/o/TV/薬屋のひとりごと/Season 03/The Apothecary Diaries - S03E01 - Locusts.mp4"
            )
        );
        let mut no_orig = ep.clone();
        no_orig.retain(|k, _| !k.starts_with("original"));
        assert!(
            destination(
                Some(Path::new("/o")),
                Path::new("a.mp4"),
                &no_orig,
                &cfg,
                &sw
            )
            .starts_with("/o/TV/The Apothecary Diaries")
        );
        // NFC: decomposed e + combining acute -> é
        assert_eq!(
            safe_component("Poke\u{301}mon\u{1}x. ", None, &cfg, 255, true),
            "Pokémon-x"
        );
        let mv = m(&[
            ("kind", "movie"),
            ("title", "Amélie: A/B? \"x\" <y>|z\\"),
            ("year", "2001"),
        ]);
        let d = destination(Some(Path::new("/o")), Path::new("a.mkv"), &mv, &cfg, &n);
        assert_eq!(
            d,
            PathBuf::from(
                "/o/Movies/Amélie- A-B- -x- -y--z- (2001)/Amélie- A-B- -x- -y--z- (2001).mkv"
            )
        );
        // bytes: cut at a char boundary, extension kept
        let s = safe_component(
            &format!("{}.mp4", "薬".repeat(100)),
            Some("mp4"),
            &cfg,
            20,
            true,
        );
        assert_eq!(s, format!("{}.mp4", "薬".repeat(5)));
        // chars (macOS): extension kept, trailing space/dash/dot trimmed
        assert_eq!(
            safe_component(
                "Show S01E01 - A very long title.mkv",
                Some("mkv"),
                &cfg,
                18,
                false
            ),
            "Show S01E01.mkv"
        );
        assert_eq!(
            safe_component(
                &format!("{}.mp4", "薬".repeat(300)),
                Some("mp4"),
                &cfg,
                255,
                false
            ),
            format!("{}.mp4", "薬".repeat(251))
        );
        assert_eq!(
            safe_component(
                &format!("{}.mp4", "薬".repeat(300)),
                Some("mp4"),
                &cfg,
                255,
                true
            ),
            format!("{}.mp4", "薬".repeat(83))
        );
        assert_eq!(
            safe_component("abc.mp4", Some("mp4"), &cfg, 0, true),
            "abc.mp4"
        ); // 0 = no limit
        // default: 255, chars on macOS, bytes elsewhere; files and folders
        assert_eq!(
            (n.filename_max, n.in_bytes()),
            (255, !cfg!(target_os = "macos"))
        );
        assert!(
            !NamingCfg {
                filename_unit: "chars".into(),
                ..n.clone()
            }
            .in_bytes()
                && NamingCfg {
                    filename_unit: "bytes".into(),
                    ..n.clone()
                }
                .in_bytes()
        );
        let long = m(&[
            ("kind", "episode"),
            ("show", &"S".repeat(300)),
            ("season", "1"),
            ("episode", "1"),
            ("episode_title", "T"),
        ]);
        let d = destination(Some(Path::new("/o")), Path::new("a.mkv"), &long, &cfg, &n);
        assert!(d.file_name().unwrap().len() == 255 && d.to_string_lossy().ends_with(".mkv"));
        assert_eq!(
            d.parent()
                .unwrap()
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .len(),
            255
        );
    }

    #[test]
    fn default_templates_roots_and_override() {
        let all = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let (cfg, n) = (all.sort, all.naming);
        assert!(!cfg.enabled, "sorting must stay opt-in");
        let home = std::env::var("HOME").unwrap();
        let ep = m(&[
            ("kind", "episode"),
            ("show", "The Apothecary Diaries"),
            ("original_show", "薬屋のひとりごと"),
            ("year", "2023"),
            ("season", "3"),
            ("episode", "1"),
            ("episode_title", "Locusts"),
            ("original_episode_title", "蝗"),
        ]);
        let d = destination(None, Path::new("a.mp4"), &ep, &cfg, &n);
        assert_eq!(
            d,
            PathBuf::from(format!(
                "{home}/TV/The Apothecary Diaries (2023)/Season 3/薬屋のひとりごと S03E01 - 蝗.mp4"
            ))
        );
        let mv = m(&[("kind", "movie"), ("title", "Evolution"), ("year", "2001")]);
        let d = destination(None, Path::new("x.mp4"), &mv, &cfg, &n);
        assert_eq!(
            d,
            PathBuf::from(format!("{home}/Movies/Evolution/Evolution (2001).mp4"))
        );
        // -o replaces both roots
        assert_eq!(
            destination(Some(Path::new("/o")), Path::new("x.mp4"), &mv, &cfg, &n),
            PathBuf::from("/o/Evolution/Evolution (2001).mp4")
        );
        assert!(
            destination(Some(Path::new("/o")), Path::new("a.mp4"), &ep, &cfg, &n)
                .starts_with("/o/The Apothecary Diaries (2023)/Season 3")
        );
        // ~ in a custom root
        let c2 = SortCfg {
            tv_root: "~/Shows".into(),
            ..cfg.clone()
        };
        assert!(
            destination(None, Path::new("a.mp4"), &ep, &c2, &n)
                .starts_with(format!("{home}/Shows/"))
        );
    }
}
