//! Config loading: built-in defaults < main config < sidecar < CLI flags.
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG: &str = include_str!("../config/jack.toml");

/// Per-file options (main config top level, sidecar, flags); all optional so layers merge.
#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
    pub sidecar_suffix: Option<String>,
    pub meta_suffix: Option<String>,
    pub output: Option<PathBuf>,
    pub dry_run: Option<bool>,
    pub download_only: Option<bool>,
    pub from_file: Option<PathBuf>,
    pub imdb: Option<String>,
    pub thetvdb: Option<String>,
    pub tmdb: Option<String>,
    pub youtube: Option<String>,
}

impl Settings {
    /// Fill unset fields of `self` from `lower`.
    pub fn over(self, lower: &Settings) -> Settings {
        let l = lower.clone();
        Settings {
            extensions: self.extensions.or(l.extensions),
            recursive: self.recursive.or(l.recursive),
            sidecar_suffix: self.sidecar_suffix.or(l.sidecar_suffix),
            meta_suffix: self.meta_suffix.or(l.meta_suffix),
            output: self.output.or(l.output),
            dry_run: self.dry_run.or(l.dry_run),
            download_only: self.download_only.or(l.download_only),
            from_file: self.from_file.or(l.from_file),
            imdb: self.imdb.or(l.imdb),
            thetvdb: self.thetvdb.or(l.thetvdb),
            tmdb: self.tmdb.or(l.tmdb),
            youtube: self.youtube.or(l.youtube),
        }
    }
    pub fn flag(v: Option<bool>) -> bool {
        v.unwrap_or(false)
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct TaggerCfg {
    pub mp4_tool: String,
    pub atomicparsley: String,
    pub exiftool: String,
    pub mkvpropedit: String,
    pub mp4_extensions: Vec<String>,
    pub mkv_extensions: Vec<String>,
    pub embed_artwork: bool,
    pub short_description_max: usize,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub base: Settings,
    pub tagger: TaggerCfg,
}

pub fn expand(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match (s.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(h)) if rest.is_empty() || rest.starts_with('/') => {
            PathBuf::from(format!("{}{rest}", h.to_string_lossy()))
        }
        _ => p.to_path_buf(),
    }
}

fn expand_s(s: &mut String) {
    *s = expand(Path::new(s)).to_string_lossy().into_owned();
}

fn merge(a: &mut toml::Value, b: toml::Value) {
    match (a, b) {
        (toml::Value::Table(a), toml::Value::Table(b)) => {
            for (k, v) in b {
                match a.get_mut(&k) {
                    Some(x) => merge(x, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (a, b) => *a = b,
    }
}

fn read(p: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Main config deep-merged over the built-in defaults.
pub fn load(explicit: Option<&Path>) -> Result<Config, String> {
    let mut v: toml::Value = toml::from_str(DEFAULT_CONFIG).map_err(|e| format!("built-in config: {e}"))?;
    let path = match explicit {
        Some(p) => Some(expand(p)),
        None => std::env::var_os("HOME").map(|h| Path::new(&h).join(".config/jack/config.toml")),
    };
    if let Some(p) = path {
        match read(&p)? {
            Some(t) => merge(&mut v, toml::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?),
            None if explicit.is_some() => return Err(format!("{}: not found", p.display())),
            None => {}
        }
    }
    let mut c: Config = v.try_into().map_err(|e| format!("config: {e}"))?;
    for s in [&mut c.tagger.atomicparsley, &mut c.tagger.exiftool, &mut c.tagger.mkvpropedit] {
        expand_s(s);
    }
    Ok(c)
}

pub fn suffixed(media: &Path, suffix: &str) -> PathBuf {
    let mut s = media.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

pub fn load_sidecar(p: &Path) -> Result<Option<Settings>, String> {
    read(p)?
        .map(|t| toml::from_str(&t).map_err(|e| format!("{}: {e}", p.display())))
        .transpose()
}

/// Effective settings for one file: flags > sidecar > main config. Returns (settings, had_sidecar).
pub fn resolve(flags: &Settings, main: &Settings, media: &Path) -> Result<(Settings, bool), String> {
    let suffix = main.sidecar_suffix.clone().unwrap_or_else(|| ".config".into());
    let side = load_sidecar(&suffixed(media, &suffix))?;
    let has = side.is_some();
    let mut s = flags.clone().over(&side.unwrap_or_default().over(main));
    s.output = s.output.as_deref().map(expand);
    s.from_file = s.from_file.as_deref().map(expand);
    Ok((s, has))
}

pub fn has_ext(p: &Path, exts: &[String]) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| x.trim_start_matches('.').eq_ignore_ascii_case(e)))
}
