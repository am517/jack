mod config;
mod meta;
mod tagger;

use clap::Parser;
use config::{Config, Settings, has_ext, suffixed};
use meta::Meta;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

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
    /// Fetch metadata/artwork only (saved beside the file); don't modify media
    #[arg(short, long)]
    download_only: bool,
    /// Apply previously downloaded metadata (a .meta.json file, or a directory of them)
    #[arg(short, long)]
    from_file: Option<PathBuf>,
    /// Destination root for sorted files
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Show what would change; change nothing
    #[arg(long)]
    dry_run: bool,
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

fn scan(paths: &[PathBuf], s: &Settings) -> Vec<PathBuf> {
    let exts = s.extensions.clone().unwrap_or_default();
    let depth = if s.recursive.unwrap_or(true) { usize::MAX } else { 1 };
    let mut out = Vec::new();
    for p in paths {
        let p = config::expand(p);
        if p.is_file() {
            out.push(p); // explicitly named files are always included
        } else {
            for e in walkdir::WalkDir::new(&p).max_depth(depth).sort_by_file_name().into_iter().flatten() {
                if e.file_type().is_file() && has_ext(e.path(), &exts) {
                    out.push(e.into_path());
                }
            }
        }
    }
    out
}

/// Metadata for one file from online providers.
fn fetch(_path: &Path, _s: &Settings, _cfg: &Config) -> Result<Meta, String> {
    Err("no metadata providers yet (phase 3); use --from-file".into())
}

fn process(path: &Path, s: &Settings, cfg: &Config, out: &mut String) -> Result<(), String> {
    let dry = Settings::flag(s.dry_run);
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
    let meta = match &s.from_file {
        Some(f) if f.is_dir() => meta::load(&f.join(format!("{}{msuf}", path.file_name().unwrap().to_string_lossy())))?,
        Some(f) => meta::load(f)?,
        None => fetch(path, s, cfg)?,
    };
    if Settings::flag(s.download_only) {
        let mp = suffixed(path, msuf);
        if !dry {
            meta::save(&mp, &meta)?;
        }
        let _ = writeln!(out, "    save metadata: {}", mp.display());
        return Ok(());
    }
    let cmd = tagger::command(path, &meta, &cfg.tagger)?;
    let _ = writeln!(out, "    tag: {}", tagger::show(&cmd));
    if !dry {
        tagger::run(&cmd)?;
    }
    Ok(())
}

fn ids(s: &Settings) -> String {
    let v: Vec<String> = [("imdb", &s.imdb), ("tmdb", &s.tmdb), ("thetvdb", &s.thetvdb), ("youtube", &s.youtube)]
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
        .collect();
    if v.is_empty() { "-".into() } else { v.join(" ") }
}

/// Returns (report text, number of failed files).
fn run(cli: &Cli) -> Result<(String, usize), String> {
    let cfg = config::load(cli.config.as_deref())?;
    let flags = Settings::from(cli);
    let files = scan(&cli.paths, &cfg.base);
    let dry = cli.dry_run || Settings::flag(cfg.base.dry_run);
    let mut o = format!("jack{}: {} media file(s) found\n", if dry { " dry run" } else { "" }, files.len());
    let mut failed = 0;
    for p in files {
        let r = config::resolve(&flags, &cfg.base, &p).and_then(|(s, side)| {
            let _ = writeln!(o, "{}{}", p.display(), if side { "  [sidecar]" } else { "" });
            let _ = writeln!(o, "    ids: {}", ids(&s));
            process(&p, &s, &cfg, &mut o)
        });
        if let Err(e) = r {
            failed += 1;
            let _ = writeln!(o, "    error: {e}");
        }
    }
    Ok((o, failed))
}

fn main() {
    let cli = Cli::parse();
    match run(&cli) {
        Ok((s, failed)) => {
            print!("{s}");
            if failed > 0 {
                std::process::exit(1);
            }
        }
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
        fs::write(&cfg, "output = \"/main\"\ntmdb = \"main\"\nimdb = \"main\"\n[tagger]\nexiftool = \"/x\"\n").unwrap();
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        fs::write(d.path().join("a.mp4.config"), "tmdb = \"side\"\nimdb = \"side\"\n").unwrap();
        let c = cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--imdb", "flag"]);
        let main = config::load(c.config.as_deref()).unwrap();
        assert_eq!(main.tagger.exiftool, "/x");
        assert_eq!(main.tagger.mkvpropedit, "mkvpropedit"); // deep-merged default
        let (s, side) = config::resolve(&Settings::from(&c), &main.base, &m).unwrap();
        assert!(side);
        assert_eq!(s.imdb.as_deref(), Some("flag"));
        assert_eq!(s.tmdb.as_deref(), Some("side"));
        assert_eq!(s.output, Some(PathBuf::from("/main")));
        assert_eq!(s.recursive, Some(true));
        assert!(s.extensions.unwrap().contains(&"mkv".to_string()));
    }

    #[test]
    fn tilde_expands() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(config::expand(Path::new("~/x")), PathBuf::from(format!("{home}/x")));
        assert_eq!(config::expand(Path::new("~bob/x")), PathBuf::from("~bob/x"));
    }

    #[test]
    fn extension_filter_case_insensitive() {
        let e = vec!["mp4".to_string(), ".mkv".to_string()];
        assert!(has_ext(Path::new("x/A.MP4"), &e));
        assert!(has_ext(Path::new("b.mkv"), &e));
        assert!(!has_ext(Path::new("b.mp4.config"), &e));
        assert!(!has_ext(Path::new("noext"), &e));
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
    fn dry_run_from_file_prints_tag_command() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "").unwrap();
        let m = d.path().join("m.m4v");
        fs::write(&m, "x").unwrap();
        fs::write(d.path().join("m.m4v.config"), "tmdb = \"603\"\n").unwrap();
        let mf = d.path().join("m.m4v.meta.json");
        fs::write(&mf, r#"{"kind":"movie","title":"The Matrix","year":"1999"}"#).unwrap();
        let c = cli(&[d.path().to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--dry-run", "-f", d.path().to_str().unwrap()]);
        let (out, failed) = run(&c).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(out.contains("1 media file(s) found") && out.contains("m.m4v  [sidecar]"), "{out}");
        assert!(out.contains("tmdb=603") && out.contains("--title 'The Matrix' --year 1999"), "{out}");
        assert_eq!(fs::read(&m).unwrap(), b"x"); // untouched
    }

    #[test]
    fn download_only_saves_meta_without_touching_media() {
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "").unwrap();
        let m = d.path().join("m.mkv");
        fs::write(&m, "x").unwrap();
        let src = d.path().join("src.json");
        fs::write(&src, r#"{"kind":"movie","title":"T"}"#).unwrap();
        let c = cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "-d", "-f", src.to_str().unwrap()]);
        let (out, failed) = run(&c).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert_eq!(meta::load(&d.path().join("m.mkv.meta.json")).unwrap()["title"], "T");
        assert_eq!(fs::read(&m).unwrap(), b"x");
    }

    #[test]
    fn missing_explicit_config_errors() {
        assert!(config::load(Some(Path::new("/nonexistent/jack.toml"))).is_err());
    }
}
