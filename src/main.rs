use clap::Parser;
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG: &str = include_str!("../config/jack.toml");

#[derive(Parser, Debug, Default)]
#[command(name = "jack", version, about = "Tag and sort movie and TV files")]
struct Cli {
    /// Files or directories to process
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Main config file (default: ~/.config/jack/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    imdb: Option<String>,
    #[arg(long)]
    thetvdb: Option<String>,
    #[arg(long)]
    tmdb: Option<String>,
    #[arg(long)]
    youtube: Option<String>,
    /// Fetch metadata/artwork only; don't modify files
    #[arg(short, long)]
    download_only: bool,
    /// Apply previously downloaded metadata
    #[arg(short, long)]
    from_file: Option<PathBuf>,
    /// Destination root for sorted files
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Show what would change
    #[arg(long)]
    dry_run: bool,
}

/// One config layer; every field optional so layers can merge.
#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
struct Settings {
    extensions: Option<Vec<String>>,
    recursive: Option<bool>,
    sidecar_suffix: Option<String>,
    output: Option<PathBuf>,
    dry_run: Option<bool>,
    download_only: Option<bool>,
    from_file: Option<PathBuf>,
    imdb: Option<String>,
    thetvdb: Option<String>,
    tmdb: Option<String>,
    youtube: Option<String>,
}

impl Settings {
    /// Fill unset fields of `self` from `lower`.
    fn over(self, lower: &Settings) -> Settings {
        let l = lower.clone();
        Settings {
            extensions: self.extensions.or(l.extensions),
            recursive: self.recursive.or(l.recursive),
            sidecar_suffix: self.sidecar_suffix.or(l.sidecar_suffix),
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
}

impl From<&Cli> for Settings {
    fn from(c: &Cli) -> Self {
        Settings {
            output: c.output.clone(),
            dry_run: c.dry_run.then_some(true),
            download_only: c.download_only.then_some(true),
            from_file: c.from_file.clone(),
            imdb: c.imdb.clone(),
            thetvdb: c.thetvdb.clone(),
            tmdb: c.tmdb.clone(),
            youtube: c.youtube.clone(),
            ..Default::default()
        }
    }
}

fn parse(text: &str, src: &Path) -> Result<Settings, String> {
    toml::from_str(text).map_err(|e| format!("{}: {e}", src.display()))
}

fn load_file(p: &Path) -> Result<Option<Settings>, String> {
    match std::fs::read_to_string(p) {
        Ok(t) => parse(&t, p).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Main config layered over built-in defaults (from config/jack.toml).
fn load_main(explicit: Option<&Path>) -> Result<Settings, String> {
    let defaults = parse(DEFAULT_CONFIG, Path::new("config/jack.toml (built-in)"))?;
    let main = match explicit {
        Some(p) => load_file(p)?.ok_or_else(|| format!("{}: not found", p.display()))?,
        None => match std::env::var_os("HOME") {
            Some(h) => load_file(&Path::new(&h).join(".config/jack/config.toml"))?.unwrap_or_default(),
            None => Settings::default(),
        },
    };
    Ok(main.over(&defaults))
}

fn sidecar_path(media: &Path, suffix: &str) -> PathBuf {
    let mut s = media.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn is_media(p: &Path, exts: &[String]) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| x.trim_start_matches('.').eq_ignore_ascii_case(e)))
}

fn scan(paths: &[PathBuf], s: &Settings) -> Vec<PathBuf> {
    let exts = s.extensions.clone().unwrap_or_default();
    let depth = if s.recursive.unwrap_or(true) { usize::MAX } else { 1 };
    let mut out = Vec::new();
    for p in paths {
        if p.is_file() {
            out.push(p.clone()); // explicitly named files are always included
        } else {
            for e in walkdir::WalkDir::new(p).max_depth(depth).sort_by_file_name().into_iter().flatten() {
                if e.file_type().is_file() && is_media(e.path(), &exts) {
                    out.push(e.into_path());
                }
            }
        }
    }
    out
}

/// Resolve effective settings per file: flags > sidecar > main > defaults.
fn resolve(cli: &Settings, main: &Settings, media: &Path) -> Result<(Settings, bool), String> {
    let suffix = main.sidecar_suffix.clone().unwrap_or_else(|| ".config".into());
    let side = load_file(&sidecar_path(media, &suffix))?;
    let has = side.is_some();
    Ok((cli.clone().over(&side.unwrap_or_default().over(main)), has))
}

fn report(files: &[(PathBuf, Settings, bool)]) -> String {
    let mut o = format!("jack dry run: {} media file(s) found\n", files.len());
    for (p, s, side) in files {
        let _ = writeln!(o, "{}{}", p.display(), if *side { "  [sidecar]" } else { "" });
        let ids: Vec<String> = [("imdb", &s.imdb), ("tmdb", &s.tmdb), ("thetvdb", &s.thetvdb), ("youtube", &s.youtube)]
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
            .collect();
        let _ = writeln!(
            o,
            "    ids: {}  output: {}  download_only: {}",
            if ids.is_empty() { "-".into() } else { ids.join(" ") },
            s.output.as_ref().map_or("-".into(), |p| p.display().to_string()),
            s.download_only.unwrap_or(false)
        );
    }
    o
}

fn run(cli: &Cli) -> Result<String, String> {
    let main = load_main(cli.config.as_deref())?;
    let flags = Settings::from(cli);
    let mut files = Vec::new();
    for p in scan(&cli.paths, &main) {
        let (s, side) = resolve(&flags, &main, &p)?;
        files.push((p, s, side));
    }
    let dry = files.iter().all(|f| f.1.dry_run.unwrap_or(false));
    if dry || files.is_empty() {
        Ok(report(&files))
    } else {
        Ok(format!(
            "{}\nphase 1: only --dry-run is implemented; nothing was changed.\n",
            report(&files)
        ))
    }
}

fn main() {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(s) => print!("{s}"),
        Err(e) => {
            eprintln!("jack: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cli(args: &[&str]) -> Cli {
        Cli::parse_from(std::iter::once("jack").chain(args.iter().copied()))
    }

    #[test]
    fn precedence_flags_sidecar_main_defaults() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "output = \"/main\"\ntmdb = \"main\"\nimdb = \"main\"\n").unwrap();
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        fs::write(d.path().join("a.mp4.config"), "tmdb = \"side\"\nimdb = \"side\"\n").unwrap();
        let c = cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--imdb", "flag"]);
        let main = load_main(c.config.as_deref()).unwrap();
        let (s, side) = resolve(&Settings::from(&c), &main, &m).unwrap();
        assert!(side);
        assert_eq!(s.imdb.as_deref(), Some("flag"));
        assert_eq!(s.tmdb.as_deref(), Some("side"));
        assert_eq!(s.output, Some(PathBuf::from("/main")));
        assert_eq!(s.recursive, Some(true)); // built-in default
        assert!(s.extensions.unwrap().contains(&"mkv".to_string()));
    }

    #[test]
    fn extension_filter_case_insensitive() {
        let e = vec!["mp4".to_string(), ".mkv".to_string()];
        assert!(is_media(Path::new("x/A.MP4"), &e));
        assert!(is_media(Path::new("b.mkv"), &e));
        assert!(!is_media(Path::new("b.mp4.config"), &e));
        assert!(!is_media(Path::new("noext"), &e));
    }

    #[test]
    fn scan_temp_dir_recursive_and_flat() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("sub")).unwrap();
        for f in ["a.mp4", "b.txt", "sub/c.mkv", "a.mp4.config"] {
            fs::write(d.path().join(f), "").unwrap();
        }
        let mut s = Settings { extensions: Some(vec!["mp4".into(), "mkv".into()]), ..Default::default() };
        assert_eq!(scan(&[d.path().into()], &s).len(), 2);
        s.recursive = Some(false);
        assert_eq!(scan(&[d.path().into()], &s), vec![d.path().join("a.mp4")]);
    }

    #[test]
    fn dry_run_output() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "").unwrap();
        fs::write(d.path().join("m.m4v"), "").unwrap();
        fs::write(d.path().join("m.m4v.config"), "tmdb = \"603\"\n").unwrap();
        let c = cli(&[d.path().to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--dry-run"]);
        let out = run(&c).unwrap();
        assert!(out.contains("1 media file(s) found"), "{out}");
        assert!(out.contains("m.m4v  [sidecar]"));
        assert!(out.contains("tmdb=603"));
        assert!(!out.contains("nothing was changed"));
    }

    #[test]
    fn missing_explicit_config_errors() {
        assert!(load_main(Some(Path::new("/nonexistent/jack.toml"))).is_err());
    }
}
