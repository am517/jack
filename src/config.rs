//! Config loading: built-in defaults < main config < sidecar < CLI flags.
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Main config location under $HOME (Windups group).
pub const MAIN_CONFIG: &str = ".config/windups/jack/config.toml";

/// Project keys embedded at build time (never committed). Config values win over these.
pub const EMBED_TMDB_TOKEN: Option<&str> = option_env!("JACK_TMDB_TOKEN");
pub const EMBED_TVDB_KEY: Option<&str> = option_env!("JACK_TVDB_KEY");
pub const EMBED_OMDB_KEY: Option<&str> = option_env!("JACK_OMDB_KEY");

/// Fill empty provider keys from embedded build-time keys (config file value > embedded key).
pub fn apply_embedded(p: &mut crate::providers::ProvidersCfg, tmdb: Option<&str>, tvdb: Option<&str>, omdb: Option<&str>) {
    if p.omdb.api_key.is_empty() {
        p.omdb.api_key = omdb.unwrap_or_default().to_string();
    }
    if p.tmdb.read_token.is_empty() && p.tmdb.api_key.is_empty() {
        p.tmdb.read_token = tmdb.unwrap_or_default().to_string();
    }
    if p.thetvdb.api_key.is_empty() {
        p.thetvdb.api_key = tvdb.unwrap_or_default().to_string();
    }
}

pub const DEFAULT_CONFIG: &str = include_str!("../config/jack.toml");

/// Per-file options (main config top level, sidecar, flags); all optional so layers merge.
#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
    pub sidecar_suffix: Option<String>,
    pub meta_suffix: Option<String>,
    pub poster_suffix: Option<String>,
    pub output: Option<PathBuf>,
    pub dry_run: Option<bool>,
    pub download_only: Option<bool>,
    /// Print composite metadata to stdout.
    pub print: Option<bool>,
    /// "text" or "json".
    pub print_format: Option<String>,
    pub from_file: Option<PathBuf>,
    pub imdb: Option<String>,
    pub thetvdb: Option<String>,
    pub tmdb: Option<String>,
    pub youtube: Option<String>,
    /// Title override (narrows the filename guess; used as the search query).
    pub name: Option<String>,
    /// Year override (search filter).
    pub year: Option<String>,
    /// "poster" or "landscape" (overrides [tagger] art_style).
    pub art_style: Option<String>,
    /// "tv", "movie" or "auto": forces the matcher and providers.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// iTunes Grouping (MP4 ©grp / MKV GROUPING).
    pub grouping: Option<String>,
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
            poster_suffix: self.poster_suffix.or(l.poster_suffix),
            output: self.output.or(l.output),
            dry_run: self.dry_run.or(l.dry_run),
            download_only: self.download_only.or(l.download_only),
            print: self.print.or(l.print),
            print_format: self.print_format.or(l.print_format),
            from_file: self.from_file.or(l.from_file),
            imdb: self.imdb.or(l.imdb),
            thetvdb: self.thetvdb.or(l.thetvdb),
            tmdb: self.tmdb.or(l.tmdb),
            youtube: self.youtube.or(l.youtube),
            name: self.name.or(l.name),
            year: self.year.or(l.year),
            art_style: self.art_style.or(l.art_style),
            kind: self.kind.or(l.kind),
            grouping: self.grouping.or(l.grouping),
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
    pub mkvextract: String,
    pub mkvmerge: String,
    pub tv_poster: Vec<String>,
    pub art_style: String,
    pub tv_landscape: Vec<String>,
    pub art_fallback: std::collections::BTreeMap<String, Vec<String>>,
    pub mkv_cover_name: String,
    /// rDNS domain ("mean") for MP4 freeform id atoms.
    pub id_domain: String,
    /// Metadata id key -> MP4 freeform atom name.
    pub mp4_id_tags: std::collections::BTreeMap<String, String>,
    /// Metadata id key -> MKV simple tag name.
    pub mkv_id_tags: std::collections::BTreeMap<String, String>,
    /// Also write `rated` as the iTunes content rating (iTunEXTC) in MP4s.
    pub mp4_content_rating: bool,
    /// --ask-art: pick the art interactively from all candidates.
    pub ask_art: bool,
    pub ask_art_max: usize,
    /// macOS: open the candidates folder with ask_art_open_command.
    /// Unset: true on macOS, false elsewhere.
    pub ask_art_open_folder: Option<bool>,
    pub ask_art_open_command: String,
    /// Keep the candidates folder (always kept with -d).
    pub ask_art_keep: bool,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Config {
    #[serde(flatten)]
    pub base: Settings,
    pub tagger: TaggerCfg,
    pub providers: crate::providers::ProvidersCfg,
    pub merge: crate::sorter::MergeCfg,
    pub matching: crate::matcher::MatchCfg,
    pub sort: crate::sorter::SortCfg,
    pub naming: crate::sorter::NamingCfg,
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
        None => std::env::var_os("HOME").map(|h| Path::new(&h).join(MAIN_CONFIG)),
    };
    if let Some(p) = path {
        match read(&p)? {
            Some(t) => merge(&mut v, toml::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?),
            None if explicit.is_some() => return Err(format!("{}: not found", p.display())),
            None => {}
        }
    }
    let mut c: Config = v.try_into().map_err(|e| format!("config: {e}"))?;
    for s in [&mut c.tagger.atomicparsley, &mut c.tagger.exiftool, &mut c.tagger.mkvpropedit, &mut c.tagger.mkvextract, &mut c.tagger.mkvmerge] {
        expand_s(s);
    }
    apply_embedded(&mut c.providers, EMBED_TMDB_TOKEN, EMBED_TVDB_KEY, EMBED_OMDB_KEY);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_key_beats_embedded_key() {
        let mut p = crate::providers::ProvidersCfg::default();
        p.thetvdb.api_key = "from-config".into();
        apply_embedded(&mut p, Some("emb-tmdb"), Some("emb-tvdb"), Some("emb-omdb"));
        assert_eq!(p.omdb.api_key, "emb-omdb");
        assert_eq!(p.thetvdb.api_key, "from-config");
        assert_eq!(p.tmdb.read_token, "emb-tmdb");
        let mut p = crate::providers::ProvidersCfg::default();
        p.tmdb.api_key = "v3".into();
        apply_embedded(&mut p, Some("emb-tmdb"), None, None);
        assert_eq!((p.tmdb.read_token.as_str(), p.thetvdb.api_key.as_str()), ("", ""));
    }
}
