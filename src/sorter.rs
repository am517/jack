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
    out
}

pub fn destination(root: &Path, file: &Path, m: &Meta, cfg: &SortCfg) -> PathBuf {
    let ext = file.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let tv = m.get("kind").is_some_and(|k| k == "episode");
    root.join(render(if tv { &cfg.tv_template } else { &cfg.movie_template }, m, &ext, cfg))
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
        let cfg = crate::config::load(Some(Path::new("config/jack.toml"))).unwrap().sort;
        let ep = m(&[("kind", "episode"), ("show", "Saiki K"), ("season", "1"), ("episode", "2"), ("episode_title", "Who/What?")]);
        let d = destination(Path::new("/out"), Path::new("x.mkv"), &ep, &cfg);
        assert_eq!(d, PathBuf::from("/out/TV/Saiki K/Season 01/Saiki K - S01E02 - Who-What-.mkv"));
        let mv = m(&[("kind", "movie"), ("title", "The Matrix"), ("year", "1999")]);
        let t = tempfile::tempdir().unwrap();
        let src = t.path().join("a.mp4");
        std::fs::write(&src, "x").unwrap();
        let dst = destination(t.path(), &src, &mv, &cfg);
        assert!(dst.ends_with("Movies/The Matrix (1999)/The Matrix (1999).mp4"));
        apply(&src, &dst, &cfg).unwrap();
        assert!(dst.exists() && !src.exists());
        std::fs::write(&src, "y").unwrap();
        assert!(apply(&src, &dst, &cfg).unwrap_err().contains("already exists"));
    }
}
