//! Walter, the patrol windup (name pending Adam's OK). Nothing incomplete is forgotten:
//! `pending` lists unfinished work; `patrol` discovers capabilities, schedules returns, and calls
//! Jack with whatever is due. No system timers: windups call windups.
pub mod caller;
pub mod config;
pub mod discover;
pub mod patrol;

use std::path::PathBuf;

pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub struct Args {
    pub db: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub all: bool,
    pub wake: Vec<String>,
}

pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        db: None,
        config: None,
        all: false,
        wake: vec![],
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{x} needs a value"));
        match x.as_str() {
            "--all" => a.all = true,
            "--db" => a.db = Some(val()?.into()),
            "--config" => a.config = Some(val()?.into()),
            "--wake" => a.wake.push(val()?),
            o => return Err(format!("unknown argument: {o}")),
        }
    }
    Ok(a)
}

fn line(p: &windups::returns::Pending) -> String {
    let when = match (&p.next_retry_at, &p.waiting_for) {
        (Some(t), Some(w)) => format!("{t} UTC or when {w} appears"),
        (Some(t), None) => format!("{t} UTC"),
        (None, Some(w)) => format!("when {w} appears"),
        (None, None) => "now".into(),
    };
    let reason = if p.reason.is_empty() { "-" } else { &p.reason };
    format!(
        "#{} {} [{}] {reason} — next: {when} (attempts {})\n",
        p.media_id,
        p.path.as_deref().unwrap_or("(no file)"),
        p.stage,
        p.attempts
    )
}

/// `pending` or `patrol`. Returns the report.
pub fn run(cmd: &str, args: &[String], caller: &dyn caller::Caller) -> Result<String, String> {
    let a = parse(args)?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cfg_path = a
        .config
        .clone()
        .or(home.map(|h| h.join(".config/windups/walter/config.toml")));
    let cfg = config::Config::load(cfg_path.as_deref())?;
    let db = match a.db {
        Some(p) => p,
        None => windups::Settings::load()?.database_path(),
    };
    let c = windups::db::open(&db)?;
    let e = |e: rusqlite::Error| e.to_string();
    let mut out = String::new();
    let woke = discover::discover(&c, &cfg, caller, &a.wake).map_err(e)?;
    if !woke.is_empty() {
        out += &format!("woke items waiting for: {}\n", woke.join(", "));
    }
    patrol::sweep(&c, &cfg.retry).map_err(e)?;
    if cmd == "pending" {
        let all = windups::returns::pending(&c).map_err(e)?;
        out += &format!("{} pending\n", all.len());
        all.iter().for_each(|p| out += &line(p));
        return Ok(out);
    }
    let items = if a.all {
        windups::returns::pending(&c)
    } else {
        windups::returns::due(&c)
    }
    .map_err(e)?;
    let files = patrol::files(&items);
    out += &format!("{} due, {} with a file present\n", items.len(), files.len());
    if !files.is_empty() {
        let jack = cfg
            .windups
            .get("jack")
            .ok_or("no command for jack in [windups]")?;
        out += &caller.call(jack, &files)?;
        patrol::touched(&c, &items).map_err(e)?;
        patrol::sweep(&c, &cfg.retry).map_err(e)?;
    }
    Ok(out)
}
