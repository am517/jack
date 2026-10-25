mod config;
mod matcher;
mod meta;
mod providers;
mod sorter;
mod tagger;

use clap::Parser;
use config::{Config, Settings, has_ext, suffixed};
use meta::Meta;
use providers::{Http, Provider, Query, Tmdb, Tvdb};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const ATTRIBUTION: &str = "Metadata provided by TheTVDB: https://thetvdb.com\n\
This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org";

#[derive(Parser, Debug, Default)]
#[command(name = "jack", version, about = "Tag and sort movie and TV files", after_help = ATTRIBUTION)]
struct Cli {
    /// Files or directories to process
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Main config file (default: ~/.config/windups/jack/config.toml)
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
    /// Title to search for (overrides the filename guess)
    #[arg(long)]
    name: Option<String>,
    /// Release / first-air year (search filter)
    #[arg(long)]
    year: Option<String>,
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
            name: c.name.clone(),
            year: c.year.clone(),
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

/// Query from the filename plus ids from flags/sidecar.
fn query(path: &Path, s: &Settings, cfg: &Config) -> Result<Query, String> {
    let mut q = matcher::parse(path, &cfg.matching)?;
    // ids written by an earlier run beat the filename guess; flags/sidecar/config ids beat both.
    let f = tagger::read_ids(path, &cfg.tagger);
    let g = |k: &str| f.get(k).cloned();
    q.imdb = s.imdb.clone().or(g("imdb_id"));
    q.tmdb = s.tmdb.clone().or(g("tmdb_series_id").map(|x| format!("/tv/{x}"))).or(g("tmdb_id"));
    q.thetvdb = s.thetvdb.clone().or(g("thetvdb_series_id")).or(g("thetvdb_id"));
    if let Some(n) = s.name.as_ref().filter(|n| !n.is_empty()) {
        q.title = n.clone();
    }
    if let Some(y) = s.year.as_ref().filter(|y| !y.is_empty()) {
        q.year = Some(y.clone());
    }
    Ok(q)
}

/// Composite metadata from all providers in [providers] order, merged per [merge].
fn fetch(q: &Query, cfg: &Config, http: &dyn Http, out: &mut String) -> Result<Meta, String> {
    let tmdb = Tmdb { cfg: cfg.providers.tmdb.clone(), http };
    let tvdb = Tvdb::new(cfg.providers.thetvdb.clone(), http);
    let (mut found, mut errs) = (vec![], vec![]);
    for name in &cfg.providers.order {
        let p: &dyn Provider = match name.as_str() {
            "tmdb" => &tmdb,
            "thetvdb" => &tvdb,
            x => return Err(format!("[providers] order: unknown provider '{x}'")),
        };
        match p.lookup(q) {
            Ok(Some(m)) => found.push((name.clone(), m)),
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
    Ok(sorter::merge(&found, &cfg.merge))
}

fn save_poster(path: &Path, s: &Settings, cfg: &Config, meta: &mut Meta, http: &dyn Http) -> Result<(), String> {
    let ep = meta.get("kind").is_some_and(|k| k == "episode");
    let url = if ep {
        cfg.tagger.tv_poster.iter().find_map(|c| meta.get(if c == "season" { "season_poster_url" } else { "poster_url" }).cloned())
    } else {
        meta.get("poster_url").cloned()
    };
    if let Some(url) = url {
        let pp = suffixed(path, s.poster_suffix.as_deref().unwrap_or(".poster.jpg"));
        std::fs::write(&pp, http.bytes(&url)?).map_err(|e| format!("{}: {e}", pp.display()))?;
        meta.insert("poster_file".into(), pp.to_string_lossy().into_owned());
    }
    Ok(())
}

fn process(path: &Path, s: &Settings, cfg: &Config, http: &dyn Http, out: &mut String) -> Result<(), String> {
    let dry = Settings::flag(s.dry_run);
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
    let meta = match &s.from_file {
        Some(f) if f.is_dir() => meta::load(&f.join(format!("{}{msuf}", path.file_name().unwrap().to_string_lossy())))?,
        Some(f) => meta::load(f)?,
        None => {
            let mut m = fetch(&query(path, s, cfg)?, cfg, http, out)?;
            if !dry && (Settings::flag(s.download_only) || cfg.tagger.embed_artwork) {
                save_poster(path, s, cfg, &mut m, http)?;
            }
            m
        }
    };
    if s.youtube.is_some() {
        let _ = writeln!(out, "    note: --youtube is not supported yet (later phase)");
    }
    if Settings::flag(s.download_only) {
        let mp = suffixed(path, msuf);
        if !dry {
            meta::save(&mp, &meta)?;
        }
        let _ = writeln!(out, "    save metadata: {}", mp.display());
        return Ok(());
    }
    let mut meta = meta;
    if let Some(i) = s.imdb.as_deref().and_then(providers::imdb_id) {
        meta.entry("imdb_id".into()).or_insert(i);
    }
    let cmd = tagger::command(path, &meta, &cfg.tagger)?;
    for c in std::iter::once(cmd).chain(tagger::extra_commands(path, &meta, &cfg.tagger)) {
        let _ = writeln!(out, "    tag: {}", tagger::show(&c));
        if !dry {
            tagger::run(&c)?;
        }
    }
    if let Some(root) = &s.output {
        let to = sorter::destination(root, path, &meta, &cfg.sort);
        let _ = writeln!(out, "    {}: {}", cfg.sort.mode, to.display());
        if !dry {
            sorter::apply(path, &to, &cfg.sort)?;
        }
    }
    Ok(())
}

fn ids(s: &Settings) -> String {
    let v: Vec<String> = [("imdb", &s.imdb), ("tmdb", &s.tmdb), ("thetvdb", &s.thetvdb), ("youtube", &s.youtube), ("name", &s.name), ("year", &s.year)]
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
        .collect();
    if v.is_empty() { "-".into() } else { v.join(" ") }
}

/// Returns (report text, number of failed files).
fn run(cli: &Cli, http: &dyn Http) -> Result<(String, usize), String> {
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
            process(&p, &s, &cfg, http, &mut o)
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
    match run(&cli, &providers::Ureq) {
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
    fn name_and_year_override_filename_guess() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("Dune.2021.mkv");
        fs::write(&m, "").unwrap();
        fs::write(d.path().join("Dune.2021.mkv.config"), "name = \"Dune Part One\"\nyear = \"1999\"\n").unwrap();
        let cfg = config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let c = cli(&[m.to_str().unwrap(), "--year", "2021"]);
        let (s, _) = config::resolve(&Settings::from(&c), &cfg.base, &m).unwrap();
        let q = query(&m, &s, &cfg).unwrap();
        assert_eq!((q.title.as_str(), q.year.as_deref()), ("Dune Part One", Some("2021"))); // sidecar name, flag year
        let q = query(&m, &Settings::default(), &cfg).unwrap();
        assert_eq!((q.title.as_str(), q.year.as_deref()), ("Dune", Some("2021")));
    }

    #[test]
    fn ids_in_file_beat_filename_but_not_flags() {
        let ok = |t: &str| std::process::Command::new("which").arg(t).output().is_ok_and(|o| o.status.success());
        if !ok("ffmpeg") || !ok("AtomicParsley") {
            return eprintln!("skip: ffmpeg/AtomicParsley missing");
        }
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("Saiki.K.S01E02.mp4");
        assert!(std::process::Command::new("ffmpeg").args(["-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=16x16:d=0.2", "-c:v", "mpeg4"]).arg(&m).status().unwrap().success());
        let cfg = config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let ids: Meta = [("tmdb_series_id", "42"), ("thetvdb_series_id", "7"), ("imdb_id", "tt5")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        tagger::run(&tagger::command(&m, &ids, &cfg.tagger).unwrap()).unwrap();
        let q = query(&m, &Settings::default(), &cfg).unwrap();
        assert_eq!((q.tmdb.as_deref(), q.thetvdb.as_deref(), q.imdb.as_deref()), (Some("/tv/42"), Some("7"), Some("tt5")));
        let s = Settings { thetvdb: Some("99".into()), ..Default::default() };
        assert_eq!(query(&m, &s, &cfg).unwrap().thetvdb.as_deref(), Some("99"));
    }

    /// Normal (non -d) runs embed art by default, TV picks the season poster, re-runs don't duplicate.
    #[test]
    fn integration_art_embedded_by_default() {
        use serde_json::json;
        use std::process::Command;
        let ok = |t: &str| Command::new("which").arg(t).output().is_ok_and(|o| o.status.success());
        if !["ffmpeg", "AtomicParsley", "exiftool", "mkvpropedit", "mkvmerge", "mkvextract"].iter().all(|t| ok(t)) {
            return eprintln!("skip art: tools missing");
        }
        let d = tempfile::tempdir().unwrap();
        let jpg = d.path().join("art.jpg");
        assert!(Command::new("ffmpeg").args(["-loglevel", "error", "-f", "lavfi", "-i", "color=c=red:s=8x8", "-frames:v", "1"]).arg(&jpg).status().unwrap().success());
        let at = format!("@{}", jpg.display());
        for (name, writer) in [("Saiki.K.S01E02.mp4", "atomicparsley"), ("Saiki.K.S01E02.m4v", "exiftool"), ("Saiki.K.S01E02.mkv", "atomicparsley")] {
            let cfgp = d.path().join("c.toml");
            fs::write(&cfgp, format!("[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img\"\n[tagger]\nmp4_tool = \"{writer}\"\n")).unwrap();
            let m = d.path().join(name);
            assert!(Command::new("ffmpeg").args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=16x16:d=0.2", "-c:v", "mpeg4"]).arg(&m).status().unwrap().success());
            let h = providers::fake::FakeHttp::new(vec![
                ("/search/tv", json!({"results":[{"id":42}]})),
                ("/tv/42/season/1/episode/2", json!({"id":9,"name":"Ep"})),
                ("/tv/42/season/1?", json!({"poster_path":"/season.jpg"})),
                ("/tv/42?", json!({"name":"Saiki K","poster_path":"/series.jpg"})),
                ("/season.jpg", json!(at.clone())),
            ]);
            let c = cli(&[m.to_str().unwrap(), "--config", cfgp.to_str().unwrap()]);
            for _ in 0..2 {
                let (out, failed) = run(&c, &h).unwrap();
                assert_eq!(failed, 0, "{out}");
            }
            assert!(!h.calls.borrow().iter().any(|c| c.contains("/series.jpg")));
            let f = m.to_str().unwrap();
            let o = |a: &[&str]| String::from_utf8_lossy(&Command::new(a[0]).args(&a[1..]).output().unwrap().stdout).into_owned();
            if name.ends_with("mkv") {
                let j: serde_json::Value = serde_json::from_str(&o(&["mkvmerge", "-J", f])).unwrap();
                let a = j["attachments"].as_array().unwrap();
                assert_eq!(a.len(), 1, "{j}");
                assert_eq!((a[0]["file_name"].as_str(), a[0]["content_type"].as_str()), (Some("cover.jpg"), Some("image/jpeg")));
            } else {
                assert!(o(&["AtomicParsley", f, "-t"]).contains("Atom \"covr\" contains: 1 piece of artwork"), "{name}");
                assert_eq!(Command::new("exiftool").args(["-b", "-CoverArt", f]).output().unwrap().stdout, fs::read(&jpg).unwrap());
            }
        }
    }

    #[test]
    fn help_shows_attribution() {
        use clap::CommandFactory;
        let h = Cli::command().render_long_help().to_string();
        assert!(h.contains("Metadata provided by TheTVDB: https://thetvdb.com"));
        assert!(h.contains("not endorsed or certified by TMDB"));
    }

    #[test]
    fn main_config_under_windups() {
        assert_eq!(config::MAIN_CONFIG, ".config/windups/jack/config.toml");
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
        let (out, failed) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(out.contains("1 media file(s) found") && out.contains("m.m4v  [sidecar]"), "{out}");
        assert!(out.contains("tmdb=603") && out.contains("--title 'The Matrix' --year 1999"), "{out}");
        let c = cli(&[d.path().to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--dry-run", "-f", d.path().to_str().unwrap(), "-o", "/srt"]);
        let (out, _) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert!(out.contains("move: /srt/Movies/The Matrix (1999)/The Matrix (1999).m4v"), "{out}");
        assert!(!Path::new("/srt").exists());
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
        let (out, failed) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert_eq!(meta::load(&d.path().join("m.mkv.meta.json")).unwrap()["title"], "T");
        assert_eq!(fs::read(&m).unwrap(), b"x");
    }

    #[test]
    fn download_only_fetches_via_providers() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n").unwrap();
        let m = d.path().join("The Matrix.mp4");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![
            ("/search/movie", json!({"results":[{"id":603}]})),
            ("/movie/603", json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg"})),
            ("/p.jpg", json!(null)),
        ]);
        let c = cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "-d"]);
        let (out, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let meta = meta::load(&d.path().join("The Matrix.mp4.meta.json")).unwrap();
        assert_eq!(meta["year"], "1999");
        assert_eq!(fs::read(d.path().join("The Matrix.mp4.poster.jpg")).unwrap(), b"JPEG");
        assert_eq!(fs::read(&m).unwrap(), b"x");
    }

    #[test]
    fn missing_explicit_config_errors() {
        assert!(config::load(Some(Path::new("/nonexistent/jack.toml"))).is_err());
    }
}
