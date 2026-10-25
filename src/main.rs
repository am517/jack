mod config;
mod matcher;
mod meta;
mod providers;
mod setcfg;
mod sorter;
mod tagger;

use clap::Parser;
use config::{Config, Settings, has_ext, suffixed};
use meta::Meta;
use providers::{Http, Omdb, Provider, Query, Tmdb, Tvdb};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const ATTRIBUTION: &str = "Metadata provided by TheTVDB: https://thetvdb.com\n\
This product uses the TMDB API but is not endorsed or certified by TMDB: https://www.themoviedb.org\n\
Ratings, rated and awards from OMDb (The Open Movie Database): https://www.omdbapi.com";

/// No Debug: holds API keys.
#[derive(Parser, Default)]
#[command(name = "jack", version, about = "Tag and sort movie and TV files", after_help = ATTRIBUTION)]
struct Cli {
    /// Files or directories to process
    paths: Vec<PathBuf>,
    /// Save the other option flags as defaults in the main config (then run, if paths are given)
    #[arg(long)]
    set: bool,
    /// Provider order, comma-separated (e.g. tmdb,thetvdb,omdb)
    #[arg(long, value_delimiter = ',')]
    providers: Option<Vec<String>>,
    /// TMDB read access token (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "TOKEN")]
    tmdb_token: Option<Option<String>>,
    /// TheTVDB API key (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "KEY")]
    tvdb_key: Option<Option<String>>,
    /// TheTVDB subscriber PIN (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "PIN")]
    tvdb_pin: Option<Option<String>>,
    /// OMDb API key (this run; with --set: saved). No value: prompt (hidden)
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "KEY")]
    omdb_key: Option<Option<String>>,
    /// MP4 tag writer: atomicparsley or exiftool
    #[arg(long, value_parser = ["atomicparsley", "exiftool"])]
    mp4_tool: Option<String>,
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
    /// Art orientation: poster or landscape
    #[arg(long, value_parser = ["poster", "landscape"])]
    art: Option<String>,
    /// File-name language: original or a language code (see [naming])
    #[arg(long)]
    file_language: Option<String>,
    /// Folder-name language: original or a language code
    #[arg(long)]
    folder_language: Option<String>,
    /// Media type: tv, movie or auto (forces filename matching and provider search)
    #[arg(long = "type", value_parser = ["tv", "movie", "auto"])]
    kind: Option<String>,
    /// Same as --type tv
    #[arg(long, conflicts_with = "movie")]
    tv: bool,
    /// Same as --type movie
    #[arg(long)]
    movie: bool,
    /// Fetch metadata/artwork only (saved beside the file); don't modify media
    #[arg(short, long, overrides_with = "no_download_only")]
    download_only: bool,
    #[arg(long, overrides_with = "download_only")]
    no_download_only: bool,
    /// Descend into subdirectories (default)
    #[arg(long, overrides_with = "no_recursive")]
    recursive: bool,
    #[arg(long, overrides_with = "recursive")]
    no_recursive: bool,
    /// Embed artwork into media (default)
    #[arg(long, overrides_with = "no_embed_artwork")]
    embed_artwork: bool,
    #[arg(long, overrides_with = "embed_artwork")]
    no_embed_artwork: bool,
    /// Print composite metadata to stdout; without -d, modify and write nothing
    #[arg(short, long, overrides_with = "no_print")]
    print: bool,
    /// Don't print (undoes a saved print = true)
    #[arg(long, overrides_with = "print")]
    no_print: bool,
    /// --print format: text or json
    #[arg(long, value_parser = ["text", "json"])]
    print_format: Option<String>,
    /// Apply previously downloaded metadata (a .meta.json file, or a directory of them)
    #[arg(short, long)]
    from_file: Option<PathBuf>,
    /// Destination root for sorted files
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Show what would change; change nothing
    #[arg(long, overrides_with = "no_dry_run")]
    dry_run: bool,
    #[arg(long, overrides_with = "dry_run")]
    no_dry_run: bool,
}

/// `--x` / `--no-x` (last wins) -> Some(true) / Some(false) / None.
fn tri(on: bool, off: bool) -> Option<bool> {
    if on { Some(true) } else if off { Some(false) } else { None }
}

impl From<&Cli> for Settings {
    fn from(c: &Cli) -> Self {
        Settings {
            output: c.output.clone(),
            dry_run: tri(c.dry_run, c.no_dry_run),
            download_only: tri(c.download_only, c.no_download_only),
            print: tri(c.print, c.no_print),
            recursive: tri(c.recursive, c.no_recursive),
            print_format: c.print_format.clone(),
            from_file: c.from_file.clone(),
            imdb: c.imdb.clone(),
            thetvdb: c.thetvdb.clone(),
            tmdb: c.tmdb.clone(),
            youtube: c.youtube.clone(),
            name: c.name.clone(),
            year: c.year.clone(),
            art_style: c.art.clone(),
            kind: c.tv.then(|| "tv".into()).or(c.movie.then(|| "movie".into())).or(c.kind.clone()),
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
    let kind = matcher::forced(s.kind.as_deref());
    let mut q = matcher::parse(path, &cfg.matching, kind)?;
    q.kind = kind.map(String::from);
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
    let pc = &cfg.providers;
    let mut tc = pc.tmdb.clone();
    tc.keep_original = pc.keep_original;
    if tc.language.is_empty() {
        tc.language = pc.language.clone();
    }
    let mut vc = pc.thetvdb.clone();
    vc.keep_original = pc.keep_original;
    if vc.language.is_empty() {
        vc.language = pc.language_codes.get(&pc.language).cloned().unwrap_or_else(|| pc.language.clone());
    }
    let tmdb = Tmdb { cfg: tc, http };
    let tvdb = Tvdb::new(vc, http);
    let omdb = Omdb { cfg: cfg.providers.omdb.clone(), http };
    let (mut found, mut errs) = (vec![], vec![]);
    for name in &cfg.providers.order {
        let p: &dyn Provider = match name.as_str() {
            "tmdb" => &tmdb,
            "thetvdb" => &tvdb,
            "omdb" => &omdb,
            x => return Err(format!("[providers] order: unknown provider '{x}'")),
        };
        match p.lookup(q) {
            Ok(Some(mut m)) => {
                if let Some(w) = m.remove(providers::WARNING) {
                    errs.push(format!("{}: {w}", p.name()));
                }
                found.push((name.clone(), m))
            }
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
    let mut m = sorter::merge(&found, &cfg.merge);
    let ep = m.get("kind").is_some_and(|k| k == "episode");
    if ep && !m.contains_key("episode_title") && cfg.matching.episode_title_from_filename {
        if let Some(t) = &q.episode_title {
            let _ = writeln!(out, "    warning: no episode title from providers; using the filename's ({t})");
            m.insert("episode_title".into(), t.clone());
        }
    }
    if ep && pc.tv_show_year {
        if let (Some(s), Some(y)) = (m.get("show").cloned(), m.get("year").cloned()) {
            m.insert("show".into(), format!("{s} ({y})"));
        }
    }
    if pc.original_in_description {
        if let (Some(o), Some(d)) = (m.get("original_title").cloned(), m.get("description").cloned()) {
            m.insert("description".into(), format!("{d}\n\nOriginal title: {o}"));
        }
    }
    Ok(m)
}

/// The art URL for the effective style (flag > sidecar > [tagger] art_style), with fallbacks.
fn art_url(meta: &Meta, s: &Settings, cfg: &Config) -> Option<String> {
    let t = &cfg.tagger;
    let ep = meta.get("kind").is_some_and(|k| k == "episode");
    let style = s.art_style.clone().unwrap_or_else(|| t.art_style.clone());
    let order = t.art_fallback.get(&style).cloned().unwrap_or_else(|| vec![style.clone()]);
    let keys = |st: &str| -> Vec<&str> {
        match (st, ep) {
            ("landscape", true) => t.tv_landscape.iter().map(|c| if c == "still" { "still_url" } else { "backdrop_url" }).collect(),
            ("landscape", false) => vec!["backdrop_url"],
            (_, true) => t.tv_poster.iter().map(|c| if c == "season" { "season_poster_url" } else { "poster_url" }).collect(),
            _ => vec!["poster_url"],
        }
    };
    order.iter().flat_map(|st| keys(st)).find_map(|k| meta.get(k).cloned())
}

fn save_poster(path: &Path, s: &Settings, cfg: &Config, meta: &mut Meta, http: &dyn Http) -> Result<(), String> {
    let url = art_url(meta, s, cfg);
    if let Some(url) = url {
        let pp = suffixed(path, s.poster_suffix.as_deref().unwrap_or(".poster.jpg"));
        std::fs::write(&pp, http.bytes(&url)?).map_err(|e| format!("{}: {e}", pp.display()))?;
        meta.insert("poster_file".into(), pp.to_string_lossy().into_owned());
    }
    Ok(())
}

/// `--print` output: (text, json entries).
#[derive(Default)]
struct Printed {
    text: String,
    json: Vec<serde_json::Value>,
}

fn print_meta(path: &Path, s: &Settings, cfg: &Config, meta: &Meta, p: &mut Printed) {
    let mut m = meta.clone();
    if let Some(u) = art_url(meta, s, cfg) {
        m.insert("art_url".into(), u);
    }
    if s.print_format.as_deref() == Some("json") {
        p.json.push(serde_json::json!({"file": path.to_string_lossy(), "metadata": m}));
    } else {
        let _ = writeln!(p.text, "{}", path.display());
        for (k, v) in &m {
            let _ = writeln!(p.text, "  {k}: {}", v.replace('\n', " "));
        }
        p.text.push('\n');
    }
}

fn process(path: &Path, s: &Settings, cfg: &Config, http: &dyn Http, out: &mut String, pr: &mut Printed) -> Result<(), String> {
    let dry = Settings::flag(s.dry_run);
    let (print, dl) = (Settings::flag(s.print), Settings::flag(s.download_only));
    let msuf = s.meta_suffix.as_deref().unwrap_or(".meta.json");
    let meta = match &s.from_file {
        Some(f) if f.is_dir() => meta::load(&f.join(format!("{}{msuf}", path.file_name().unwrap().to_string_lossy())))?,
        Some(f) => meta::load(f)?,
        None => {
            let mut m = fetch(&query(path, s, cfg)?, cfg, http, out)?;
            if !dry && (dl || (cfg.tagger.embed_artwork && !print)) {
                // art is optional: never lose the metadata over it
                if let Err(e) = save_poster(path, s, cfg, &mut m, http) {
                    let _ = writeln!(out, "    warning: artwork: {e}");
                }
            }
            m
        }
    };
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
        let to = sorter::destination(root, path, &meta, &cfg.sort, &cfg.naming);
        let _ = writeln!(out, "    {}: {}", cfg.sort.mode, to.display());
        if !dry {
            sorter::apply(path, &to, &cfg.sort)?;
        }
    }
    Ok(())
}

/// Ids in the metadata (else the requested ones).
fn found_ids(m: &Meta, s: &Settings) -> String {
    let v: Vec<String> = ["imdb_id", "tmdb_id", "tmdb_episode_id", "thetvdb_id", "thetvdb_episode_id"].iter().filter_map(|k| m.get(*k).map(|v| format!("{}={v}", k.trim_end_matches("_id")))).collect();
    if v.is_empty() { ids(s) } else { v.join(" ") }
}

fn ids(s: &Settings) -> String {
    let v: Vec<String> = [("imdb", &s.imdb), ("tmdb", &s.tmdb), ("thetvdb", &s.thetvdb), ("youtube", &s.youtube), ("name", &s.name), ("year", &s.year)]
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={v}")))
        .collect();
    if v.is_empty() { "-".into() } else { v.join(" ") }
}

/// (flag, config key) for each credential flag.
const SECRETS: [(&str, &str); 4] = [("--tmdb-token", "providers.tmdb.read_token"), ("--tvdb-key", "providers.thetvdb.api_key"), ("--tvdb-pin", "providers.thetvdb.pin"), ("--omdb-key", "providers.omdb.api_key")];

fn secret_fields(cli: &mut Cli) -> [&mut Option<Option<String>>; 4] {
    [&mut cli.tmdb_token, &mut cli.tvdb_key, &mut cli.tvdb_pin, &mut cli.omdb_key]
}

/// Fills value-less credential flags via `read` (hidden prompt in `main`).
fn resolve_secrets(cli: &mut Cli, read: &mut dyn FnMut(&str) -> Result<String, String>) -> Result<(), String> {
    for (f, (flag, _)) in secret_fields(cli).into_iter().zip(SECRETS) {
        if let Some(None) = f {
            let v = read(&format!("{flag}: "))?.trim().to_string();
            if v.is_empty() {
                return Err(format!("{flag}: empty value"));
            }
            *f = Some(Some(v));
        }
    }
    Ok(())
}

/// Hidden prompt on a terminal; otherwise one line from stdin (e.g. `echo $KEY | jack --set --omdb-key`).
fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).map_err(|e| format!("reading key: {e}"))
    } else {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).map_err(|e| format!("reading key from stdin: {e}"))?;
        Ok(s)
    }
}

fn secret_values(cli: &Cli) -> Vec<(&'static str, &'static str, String)> {
    [&cli.tmdb_token, &cli.tvdb_key, &cli.tvdb_pin, &cli.omdb_key]
        .into_iter()
        .zip(SECRETS)
        .filter_map(|(v, (f, k))| v.clone().flatten().map(|v| (f, k, v)))
        .collect()
}

fn main_config_path(cli: &Cli) -> Result<PathBuf, String> {
    match &cli.config {
        Some(p) => Ok(config::expand(p)),
        None => std::env::var_os("HOME").map(|h| Path::new(&h).join(config::MAIN_CONFIG)).ok_or("HOME not set".into()),
    }
}

/// `--set`: save option flags into the main config; per-file identifiers are skipped with a warning.
fn save_flags(cli: &Cli) -> Result<String, String> {
    use setcfg::Change;
    use toml_edit::Value;
    let mut o = String::new();
    for (f, set) in [("--imdb", cli.imdb.is_some()), ("--tmdb", cli.tmdb.is_some()), ("--thetvdb", cli.thetvdb.is_some()), ("--youtube", cli.youtube.is_some()), ("--name", cli.name.is_some()), ("--year", cli.year.is_some()), ("--from-file", cli.from_file.is_some())] {
        if set {
            let _ = writeln!(o, "warning: {f} is per-file; not saved");
        }
    }
    let s = |k, v: &str| Change { key: k, value: Value::from(v), secret: false };
    let mut ch = vec![];
    if let Some(v) = &cli.output {
        ch.push(s("output", &v.to_string_lossy()));
    }
    if let Some(v) = &cli.art {
        ch.push(s("tagger.art_style", v));
    }
    if let Some(v) = &cli.mp4_tool {
        ch.push(s("tagger.mp4_tool", v));
    }
    if let Some(v) = &cli.providers {
        ch.push(Change { key: "providers.order", value: Value::Array(v.iter().collect()), secret: false });
    }
    if let Some(v) = &Settings::from(cli).kind {
        ch.push(s("type", v));
    }
    for (k, v) in [("naming.file_language", &cli.file_language), ("naming.folder_language", &cli.folder_language)] {
        if let Some(v) = v {
            ch.push(s(k, v));
        }
    }
    if let Some(v) = &cli.print_format {
        ch.push(s("print_format", v));
    }
    let f = Settings::from(cli);
    for (k, v) in [("dry_run", f.dry_run), ("download_only", f.download_only), ("print", f.print), ("recursive", f.recursive), ("tagger.embed_artwork", tri(cli.embed_artwork, cli.no_embed_artwork))] {
        if let Some(v) = v {
            ch.push(Change { key: k, value: Value::from(v), secret: false });
        }
    }
    for (_, k, v) in secret_values(cli) {
        ch.push(Change { key: k, value: Value::from(v), secret: true });
    }
    if ch.is_empty() {
        let _ = writeln!(o, "--set: no option flags to save");
        return Ok(o);
    }
    o += &setcfg::save(&main_config_path(cli)?, config::DEFAULT_CONFIG, &ch)?;
    Ok(o)
}

/// Returns (report text, --print output, number of failed files).
fn run(cli: &Cli, http: &dyn Http) -> Result<(String, String, usize), String> {
    if let (Some(t), Some(f)) = (&cli.kind, cli.tv.then_some("tv").or(cli.movie.then_some("movie"))) {
        if t != f {
            return Err(format!("--{f} conflicts with --type {t}"));
        }
    }
    let mut pre = String::new();
    if cli.set {
        pre = save_flags(cli)?;
        if cli.paths.is_empty() {
            return Ok((pre, String::new(), 0));
        }
    } else if cli.paths.is_empty() {
        return Err("no files or directories given (see --help)".into());
    }
    let mut cfg = config::load(cli.config.as_deref())?;
    if let Some(p) = &cli.providers {
        cfg.providers.order = p.clone();
    }
    if let Some(t) = &cli.mp4_tool {
        cfg.tagger.mp4_tool = t.clone();
    }
    if let Some(v) = &cli.file_language {
        cfg.naming.file_language = v.clone();
    }
    if let Some(v) = &cli.folder_language {
        cfg.naming.folder_language = v.clone();
    }
    // credential flags beat config and embedded keys (this run)
    for (_, k, v) in secret_values(cli) {
        let p = &mut cfg.providers;
        *match k {
            "providers.tmdb.read_token" => &mut p.tmdb.read_token,
            "providers.thetvdb.api_key" => &mut p.thetvdb.api_key,
            "providers.thetvdb.pin" => &mut p.thetvdb.pin,
            _ => &mut p.omdb.api_key,
        } = v;
    }
    if let Some(v) = tri(cli.embed_artwork, cli.no_embed_artwork) {
        cfg.tagger.embed_artwork = v;
    }
    let (ib, changed) = providers::normalize_image_base(&cfg.providers.tmdb.image_base);
    if changed && !ib.is_empty() {
        pre += &format!("warning: [providers.tmdb] image_base {:?} -> {ib:?} (sizes come from poster_size etc.; update your config)\n", cfg.providers.tmdb.image_base);
        cfg.providers.tmdb.image_base = ib;
    }
    let flags = Settings::from(cli);
    let top = flags.clone().over(&cfg.base);
    let files = scan(&cli.paths, &top);
    let dry = Settings::flag(top.dry_run);
    let mut o = pre + &format!("jack{}: {} media file(s) found\n", if dry { " dry run" } else { "" }, files.len());
    let (mut failed, mut pr) = (0, Printed::default());
    for p in files {
        let r = config::resolve(&flags, &cfg.base, &p).and_then(|(s, side)| {
            let _ = writeln!(o, "{}{}", p.display(), if side { "  [sidecar]" } else { "" });
            process(&p, &s, &cfg, http, &mut o, &mut pr)
        });
        if let Err(e) = r {
            failed += 1;
            let _ = writeln!(o, "    error: {e}");
        }
    }
    if !pr.json.is_empty() {
        pr.text += &(serde_json::to_string_pretty(&pr.json).map_err(|e| e.to_string())? + "\n");
    }
    Ok((o, pr.text, failed))
}

fn main() {
    let mut cli = Cli::parse();
    if let Err(e) = resolve_secrets(&mut cli, &mut read_secret) {
        eprintln!("jack: {e}");
        std::process::exit(1);
    }
    match run(&cli, &providers::Ureq) {
        Ok((s, printed, failed)) => {
            // with --print, stdout carries only the metadata; the report goes to stderr
            if printed.is_empty() {
                print!("{s}");
            } else {
                eprint!("{s}");
                print!("{printed}");
            }
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
                let (out, _, failed) = run(&c, &h).unwrap();
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
    fn integration_landscape_art_embedded() {
        use serde_json::json;
        use std::process::Command;
        let ok = |t: &str| Command::new("which").arg(t).output().is_ok_and(|o| o.status.success());
        if !["ffmpeg", "AtomicParsley", "exiftool"].iter().all(|t| ok(t)) {
            return eprintln!("skip art: tools missing");
        }
        let d = tempfile::tempdir().unwrap();
        let jpg = d.path().join("still.jpg");
        assert!(Command::new("ffmpeg").args(["-loglevel", "error", "-f", "lavfi", "-i", "color=c=blue:s=16x9", "-frames:v", "1"]).arg(&jpg).status().unwrap().success());
        let cfgp = d.path().join("c.toml");
        fs::write(&cfgp, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img\"\nstill_size = \"w300\"\n").unwrap();
        let m = d.path().join("Saiki.K.S01E02.mp4");
        assert!(Command::new("ffmpeg").args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=16x16:d=0.2", "-c:v", "mpeg4"]).arg(&m).status().unwrap().success());
        let h = providers::fake::FakeHttp::new(vec![
            ("/search/tv", json!({"results":[{"id":42}]})),
            ("/tv/42/season/1/episode/2", json!({"id":9,"name":"Ep","still_path":"/still.jpg"})),
            ("/tv/42/season/1?", json!({"poster_path":"/season.jpg"})),
            ("/tv/42?", json!({"name":"Saiki K","poster_path":"/series.jpg","backdrop_path":"/bd.jpg"})),
            ("/w300/still.jpg", json!(format!("@{}", jpg.display()))),
        ]);
        let (out, _, failed) = run(&cli(&[m.to_str().unwrap(), "--config", cfgp.to_str().unwrap(), "--art", "landscape"]), &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(!h.calls.borrow().iter().any(|c| c.contains("season.jpg") || c.contains("bd.jpg")));
        assert_eq!(Command::new("exiftool").args(["-b", "-CoverArt"]).arg(&m).output().unwrap().stdout, fs::read(&jpg).unwrap());
    }

    #[test]
    fn art_style_choice_and_fallbacks() {
        let cfg = config::load(Some(Path::new("config/jack.toml"))).unwrap();
        let mk = |kv: &[(&str, &str)]| -> Meta { kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() };
        let movie = mk(&[("kind", "movie"), ("poster_url", "P"), ("backdrop_url", "B")]);
        let land = Settings { art_style: Some("landscape".into()), ..Default::default() };
        assert_eq!(art_url(&movie, &Settings::default(), &cfg).as_deref(), Some("P")); // poster default
        assert_eq!(art_url(&movie, &land, &cfg).as_deref(), Some("B"));
        let ep = mk(&[("kind", "episode"), ("poster_url", "SP"), ("season_poster_url", "SEA"), ("backdrop_url", "SB"), ("still_url", "ST")]);
        assert_eq!(art_url(&ep, &land, &cfg).as_deref(), Some("ST"));
        let mut e2 = ep.clone();
        e2.remove("still_url");
        assert_eq!(art_url(&e2, &land, &cfg).as_deref(), Some("SB"));
        e2.remove("backdrop_url");
        assert_eq!(art_url(&e2, &land, &cfg).as_deref(), Some("SEA")); // no landscape -> poster
        assert_eq!(art_url(&mk(&[("kind", "movie"), ("backdrop_url", "B")]), &Settings::default(), &cfg).as_deref(), Some("B"));
        // flag > sidecar > config
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "[tagger]\nart_style = \"landscape\"\n").unwrap();
        let cfg = config::load(Some(&c)).unwrap();
        let eff = |args: &[&str]| {
            let (s, _) = config::resolve(&Settings::from(&cli(&[&[m.to_str().unwrap()], args].concat())), &cfg.base, &m).unwrap();
            art_url(&movie, &s, &cfg)
        };
        assert_eq!(eff(&[]).as_deref(), Some("B"));
        fs::write(d.path().join("a.mp4.config"), "art_style = \"poster\"\n").unwrap();
        assert_eq!(eff(&[]).as_deref(), Some("P"));
        assert_eq!(eff(&["--art", "landscape"]).as_deref(), Some("B"));
    }

    #[test]
    fn set_saves_flags_keeps_comments_and_is_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("sub/c.toml");
        let cs = c.to_str().unwrap();
        // missing file: created from the template; identifiers skipped with a warning
        let (o, _, _) = run(&cli(&["--set", "--config", cs, "--art", "landscape", "-o", "/out", "--providers", "tmdb,omdb", "--mp4-tool", "exiftool", "--imdb", "tt1", "--name", "X"]), &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert!(o.contains("warning: --imdb is per-file; not saved") && o.contains("warning: --name is per-file"), "{o}");
        assert!(o.contains("tagger.art_style: \"poster\" -> \"landscape\"") && o.contains("output: (unset) -> \"/out\""), "{o}");
        assert!(!o.contains("media file"), "save-only must not run: {o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(t.contains("# Art orientation: \"poster\" or \"landscape\"") && t.contains("art_style = \"landscape\""));
        assert!(t.contains("order = [\"tmdb\", \"omdb\"]") && !t.contains("tt1"));
        let cfg = config::load(Some(&c)).unwrap();
        assert_eq!((cfg.tagger.mp4_tool.as_str(), cfg.base.output.as_deref()), ("exiftool", Some(Path::new("/out"))));
        // idempotent
        let (o, _, _) = run(&cli(&["--set", "--config", cs, "--art", "landscape"]), &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert!(o.contains("tagger.art_style: unchanged") && o.contains("no changes"), "{o}");
        assert_eq!(fs::read_to_string(&c).unwrap(), t);
        // save + run
        let m = d.path().join("a.mp4");
        fs::write(&m, "").unwrap();
        let (o, _, _) = run(&cli(&["--set", "--config", cs, "--dry-run", m.to_str().unwrap()]), &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert!(o.contains("dry_run: false -> true") && o.contains("1 media file(s) found"), "{o}");
    }

    #[test]
    fn credential_flags_run_only_set_masked_and_prompted() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "# my keys\n[providers]\norder = [\"omdb\"]\n[providers.omdb]\napi_key = \"CFGKEY1234\" # keep me\nbase_url = \"http://o/\"\n").unwrap();
        let m = d.path().join("Dune (1984).mp4");
        fs::write(&m, "").unwrap();
        let (cs, ms) = (c.to_str().unwrap(), m.to_str().unwrap());
        let h = providers::fake::FakeHttp::new(vec![("http://o/", json!({"Title":"Dune","Year":"1984"}))]);
        // run-only: flag beats config; config untouched; key never printed
        let (o, _, _) = run(&cli(&["--config", cs, "--dry-run", "--omdb-key=FLAGKEY5678", ms]), &h).unwrap();
        assert!(h.calls.borrow().iter().any(|u| u.contains("apikey=FLAGKEY5678")));
        assert!(!o.contains("FLAGKEY5678") && !o.contains("CFGKEY1234"), "{o}");
        assert!(fs::read_to_string(&c).unwrap().contains("CFGKEY1234"));
        // value-less flag prompts through the reader; --set saves masked
        let mut a = cli(&["--set", "--config", cs, "--omdb-key", "--tvdb-pin=9999"]);
        let mut asked = vec![];
        resolve_secrets(&mut a, &mut |p: &str| {
            asked.push(p.to_string());
            Ok("NEWKEY0001\n".into())
        })
        .unwrap();
        assert_eq!(asked, ["--omdb-key: "]);
        let (o, _, _) = run(&a, &h).unwrap();
        assert!(o.contains("providers.omdb.api_key: ******** -> ********") && o.contains("providers.thetvdb.pin: (unset) -> ********"), "{o}");
        assert!(!o.contains("NEWKEY0001") && !o.contains("CFGKEY1234") && !o.contains("9999"), "{o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(t.contains("api_key = \"NEWKEY0001\" # keep me") && t.starts_with("# my keys") && t.contains("pin = \"9999\""), "{t}");
        let mut e = cli(&["--omdb-key"]);
        assert!(resolve_secrets(&mut e, &mut |_: &str| Ok(" \n".into())).is_err());
    }

    #[test]
    fn help_shows_attribution() {
        use clap::CommandFactory;
        let h = Cli::command().render_long_help().to_string();
        assert!(h.contains("Metadata provided by TheTVDB: https://thetvdb.com"));
        assert!(h.contains("not endorsed or certified by TMDB") && h.contains("omdbapi.com"));
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
        let (out, _, failed) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert_eq!(failed, 0, "{out}");
        assert!(out.contains("1 media file(s) found") && out.contains("m.m4v  [sidecar]"), "{out}");
        assert!(out.contains("tmdb=603") && out.contains("--title 'The Matrix' --year 1999"), "{out}");
        let c = cli(&[d.path().to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--dry-run", "-f", d.path().to_str().unwrap(), "-o", "/srt"]);
        let (out, _, _) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
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
        let (out, _, failed) = run(&c, &providers::fake::FakeHttp::new(vec![])).unwrap();
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
        let (out, _, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let meta = meta::load(&d.path().join("The Matrix.mp4.meta.json")).unwrap();
        assert_eq!(meta["year"], "1999");
        assert_eq!(fs::read(d.path().join("The Matrix.mp4.poster.jpg")).unwrap(), b"JPEG");
        assert_eq!(fs::read(&m).unwrap(), b"x");
    }

    #[test]
    fn print_text_json_combined_with_d_and_set() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"SECRETTOK\"\nbase_url = \"http://t\"\n").unwrap();
        let cs = cfg.to_str().unwrap();
        let m = d.path().join("The Matrix.mp4");
        fs::write(&m, "x").unwrap();
        let ms = m.to_str().unwrap();
        let h = || providers::fake::FakeHttp::new(vec![
            ("/search/movie", json!({"results":[{"id":603}]})),
            ("/movie/603", json!({"title":"The Matrix","release_date":"1999-03-30","poster_path":"/p.jpg","imdb_id":"tt0133093"})),
            ("/p.jpg", json!(null)),
        ]);
        let listing = || { let mut v: Vec<_> = fs::read_dir(d.path()).unwrap().map(|e| e.unwrap().file_name()).collect(); v.sort(); v };
        let before = listing();
        // text, print only: nothing written, media untouched
        let (_, p, failed) = run(&cli(&["--config", cs, "-p", ms]), &h()).unwrap();
        assert_eq!(failed, 0);
        assert!(p.starts_with(ms) && p.contains("  title: The Matrix\n") && p.contains("  imdb_id: tt0133093") && p.contains("art_url: ") && p.contains("/p.jpg"), "{p}");
        assert!(!p.contains("SECRETTOK"));
        assert_eq!(listing(), before);
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // json is clean stdout
        let (_, p, _) = run(&cli(&["--config", cs, "--print", "--print-format", "json", ms]), &h()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert_eq!(v[0]["file"], ms);
        assert_eq!(v[0]["metadata"]["year"], "1999");
        assert_eq!(listing(), before);
        // -d + --print: both
        let (_, p, _) = run(&cli(&["--config", cs, "-d", "-p", ms]), &h()).unwrap();
        assert!(p.contains("title: The Matrix"));
        assert_eq!(meta::load(&d.path().join("The Matrix.mp4.meta.json")).unwrap()["title"], "The Matrix");
        assert!(d.path().join("The Matrix.mp4.poster.jpg").exists());
        assert_eq!(fs::read(&m).unwrap(), b"x");
        // --set saves print + print_format; config then drives json
        run(&cli(&["--set", "--config", cs, "-p", "--print-format", "json"]), &h()).unwrap();
        let t = fs::read_to_string(&cfg).unwrap();
        assert!(t.contains("print = true") && t.contains("print_format = \"json\""), "{t}");
        let (_, p, _) = run(&cli(&["--config", cs, ms]), &h()).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&p).unwrap().is_array());
    }

    #[test]
    fn type_flags_conflicts_and_set() {
        assert!(Cli::try_parse_from(["jack", "--tv", "--movie", "x"]).is_err());
        assert!(run(&cli(&["--type", "movie", "--tv", "x"]), &providers::fake::FakeHttp::new(vec![])).unwrap_err().contains("conflicts"));
        assert_eq!(Settings::from(&cli(&["--movie", "x"])).kind.as_deref(), Some("movie"));
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        run(&cli(&["--set", "--config", c.to_str().unwrap(), "--tv"]), &providers::fake::FakeHttp::new(vec![])).unwrap();
        assert!(fs::read_to_string(&c).unwrap().contains("type = \"tv\""));
        assert_eq!(config::load(Some(&c)).unwrap().base.kind.as_deref(), Some("tv"));
    }

    #[test]
    fn no_flags_last_wins_and_set_saves_false() {
        let f = Settings::from(&cli(&["--print", "--no-print", "--no-dry-run", "--dry-run", "x"]));
        assert_eq!((f.print, f.dry_run, f.download_only), (Some(false), Some(true), None));
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        let cs = c.to_str().unwrap();
        let none = || providers::fake::FakeHttp::new(vec![]);
        run(&cli(&["--set", "--config", cs, "-p", "--no-embed-artwork"]), &none()).unwrap();
        let (o, _, _) = run(&cli(&["--set", "--config", cs, "--no-print", "--no-recursive", "--no-dry-run"]), &none()).unwrap();
        assert!(o.contains("print: true -> false"), "{o}");
        let t = fs::read_to_string(&c).unwrap();
        assert!(t.contains("print = false") && t.contains("recursive = false") && t.contains("embed_artwork = false") && t.contains("dry_run = false"), "{t}");
        // saved print = true is undone per run by --no-print; --recursive beats saved false
        run(&cli(&["--set", "--config", cs, "-p"]), &none()).unwrap();
        let sub = d.path().join("a/b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("m.mkv"), "x").unwrap();
        fs::write(d.path().join("src.json"), r#"{"kind":"movie","title":"T"}"#).unwrap();
        let src = d.path().join("src.json");
        let dir = d.path().join("a");
        let args = |extra: &[&str]| { let mut v = vec![dir.to_str().unwrap(), "--config", cs, "--dry-run", "-f", src.to_str().unwrap()]; v.extend(extra); cli(&v) };
        let (o, p, _) = run(&args(&[]), &none()).unwrap();
        assert!(o.contains("0 media file(s)") && p.is_empty(), "{o}");
        let (o, p, _) = run(&args(&["--recursive", "--no-print"]), &none()).unwrap();
        assert!(o.contains("1 media file(s)") && o.contains("tag: ") && p.is_empty(), "{o}");
        let (_, p, _) = run(&args(&["--recursive"]), &none()).unwrap();
        assert!(p.contains("title: T"));
    }

    #[test]
    fn missing_explicit_config_errors() {
        assert!(config::load(Some(Path::new("/nonexistent/jack.toml"))).is_err());
    }
    #[test]
    fn apothecary_real_world_regressions() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, "[providers]\norder = [\"tmdb\", \"thetvdb\"]\nlanguage = \"en\"\nkeep_original = true\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\nimage_base = \"http://img/t/p/original/\"\n[providers.thetvdb]\napi_key = \"k\"\nbase_url = \"http://v\"\n").unwrap();
        let m = d.path().join("The Apothecary Diaries - S03E01 - Locusts.mp4");
        fs::write(&m, "x").unwrap();
        let h = providers::fake::FakeHttp::new(vec![
            ("http://t/search/tv", json!({"results":[{"id":220542}]})),
            ("http://t/tv/220542/episode_groups", json!({"results":[]})),
            ("http://t/tv/220542?", json!({"name":"The Apothecary Diaries","original_name":"薬屋のひとりごと","first_air_date":"2023-10-22","poster_path":"/p.jpg","seasons":[{"season_number":1,"episode_count":24},{"season_number":2,"episode_count":24},{"season_number":3,"episode_count":12}]})),
            ("/login", json!({"data":{"token":"T"}})),
            ("http://v/search", json!({"data":[{"tvdb_id":"482073","name":"薬屋のひとりごと (2027)","year":"2027"},{"tvdb_id":"433631","name":"薬屋のひとりごと","year":"2023","translations":{"eng":"The Apothecary Diaries"}}]})),
            ("/series/433631/translations/eng", json!({"data":{"name":"The Apothecary Diaries","overview":"Maomao..."}})),
            ("/series/433631/episodes", json!({"data":{"episodes":[{"id":777,"name":"蝗害","seasonNumber":3,"number":1}]}})),
            ("/episodes/777/translations/eng", json!({"data":{"name":"Locusts"}})),
            ("/series/433631/extended", json!({"data":{}})),
            ("/series/433631", json!({"data":{"id":433631,"name":"薬屋のひとりごと","firstAired":"2023-10-22","year":"2027"}})),
        ]);
        let c = cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "-d", "-p", "-o", "/out"]);
        let (out, p, failed) = run(&c, &h).unwrap();
        assert_eq!(failed, 0, "{out}");
        let meta = meta::load(&d.path().join("The Apothecary Diaries - S03E01 - Locusts.mp4.meta.json")).unwrap();
        assert_eq!(meta["show"], "The Apothecary Diaries");
        assert_eq!(meta["original_show"], "薬屋のひとりごと");
        assert_eq!(meta["year"], "2023", "firstAired, not the 2027 duplicate");
        assert_eq!(meta["episode_title"], "Locusts");
        assert_eq!(meta["original_episode_title"], "蝗害");
        assert!(out.contains("warning: tmdb: no S03E01"), "{out}");
        assert!(out.contains("tmdb=220542") && out.contains("433631"), "{out}");
        assert_eq!(out.matches("image_base").count(), 1, "{out}");
        assert!(out.contains("warning: artwork:"), "art failure is a warning: {out}");
        assert!(p.contains("original_show: 薬屋のひとりごと"), "{p}");
        let (out, _, _) = run(&cli(&[m.to_str().unwrap(), "--config", cfg.to_str().unwrap(), "--dry-run", "-o", "/out"]), &h).unwrap();
        assert!(out.contains("/out/TV/The Apothecary Diaries/Season 03/薬屋のひとりごと - S03E01 - 蝗害.mp4"), "{out}");
    }

}
