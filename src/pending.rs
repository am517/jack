//! `jack pending` and `jack retry`: unfinished work is never forgotten.
//! pending: every entry that isn't complete, with its stage, reason and next return.
//! retry: re-runs what is due (`--all`: everything pending). Before either, capabilities that are
//! present now (registered windups such as sammy, configured providers) wake what waits for them.
use crate::providers::Http;
use std::path::{Path, PathBuf};

pub(crate) struct Args {
    pub config: Option<PathBuf>,
    pub all: bool,
    pub wake: Vec<String>,
}

pub(crate) fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        config: None,
        all: false,
        wake: vec![],
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        match x.as_str() {
            "--all" => a.all = true,
            "--config" => a.config = Some(it.next().ok_or("--config needs a path")?.into()),
            "--wake" => a
                .wake
                .push(it.next().ok_or("--wake needs a capability")?.clone()),
            o => {
                return Err(format!(
                    "unknown argument: {o} (use --config PATH, --all, --wake CAPABILITY)"
                ));
            }
        }
    }
    Ok(a)
}

/// Capabilities present now: registered windups and configured providers, plus any named.
fn wake_present(
    c: &rusqlite::Connection,
    cfg: &crate::config::Config,
    extra: &[String],
) -> Result<Vec<String>, String> {
    let e = |e: rusqlite::Error| e.to_string();
    let mut caps: Vec<String> = c
        .prepare("SELECT name FROM windups")
        .map_err(e)?
        .query_map([], |r| r.get(0))
        .map_err(e)?
        .collect::<Result<_, _>>()
        .map_err(e)?;
    caps.extend(cfg.providers.order.iter().cloned());
    caps.extend(extra.iter().cloned());
    let mut woke = vec![];
    for cap in caps {
        if windups::returns::wake(c, &cap).map_err(e)? > 0 {
            woke.push(cap);
        }
    }
    Ok(woke)
}

fn line(p: &windups::returns::Pending) -> String {
    let when = match (&p.next_retry_at, &p.waiting_for) {
        (Some(t), Some(w)) => format!("{t} UTC or when {w} appears"),
        (Some(t), None) => format!("{t} UTC"),
        (None, Some(w)) => format!("when {w} appears"),
        (None, None) => "now".into(),
    };
    format!(
        "#{} {} [{}] {} — next: {when} (attempts {})\n",
        p.media_id,
        p.path.as_deref().unwrap_or("(no file)"),
        p.stage,
        if p.reason.is_empty() { "-" } else { &p.reason },
        p.attempts
    )
}

/// Returns (report, failed count).
pub(crate) fn run(cmd: &str, args: &[String], http: &dyn Http) -> Result<(String, usize), String> {
    let a = parse(args)?;
    let cfg = crate::config::load(a.config.as_deref())?;
    let c = windups::db::open(&crate::store::db_path(&cfg.store)?)?;
    let e = |e: rusqlite::Error| e.to_string();
    let mut out = String::new();
    let woke = wake_present(&c, &cfg, &a.wake)?;
    if !woke.is_empty() {
        out += &format!("woke items waiting for: {}\n", woke.join(", "));
    }
    if cmd == "pending" {
        let all = windups::returns::pending(&c).map_err(e)?;
        out += &format!("{} pending\n", all.len());
        all.iter().for_each(|p| out += &line(p));
        return Ok((out, 0));
    }
    let items = if a.all {
        windups::returns::pending(&c)
    } else {
        windups::returns::due(&c)
    }
    .map_err(e)?;
    drop(c);
    let paths: Vec<PathBuf> = items
        .iter()
        .filter_map(|p| p.path.as_deref())
        .map(PathBuf::from)
        .filter(|p| Path::new(p).exists())
        .collect();
    out += &format!("{} due, {} with a file present\n", items.len(), paths.len());
    if paths.is_empty() {
        return Ok((out, 0));
    }
    let mut cli = crate::cli::Cli {
        paths,
        config: a.config,
        ..Default::default()
    };
    cli.paths.dedup();
    let (s, _, failed) = crate::run::run(&cli, http)?;
    Ok((out + &s, failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    use serde_json::json;
    use std::fs;

    #[test]
    fn unfinished_work_is_listed_and_comes_back() {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("m.db");
        let cfg = d.path().join("c.toml");
        fs::write(&cfg, format!("[providers]\norder = [\"tmdb\"]\n[providers.tmdb]\nread_token = \"t\"\nbase_url = \"http://t\"\n[store]\nenabled = true\ndatabase = {:?}\n[retry]\nbackoff_hours = [0, 24]\n[retry.wait_for]\nundetermined = \"sammy\"\n", db.to_str().unwrap())).unwrap();
        fs::write(d.path().join("Nothing.Like.It.S01E01.mkv"), "x").unwrap();
        let h = FakeHttp::new(vec![("query=Nothing", json!({"results":[]}))]);
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        crate::run::run(
            &crate::testutil::cli(&[
                d.path().to_str().unwrap(),
                "--config",
                cfg.to_str().unwrap(),
            ]),
            &h,
        )
        .unwrap();
        let (out, _) = run("pending", &s(&["--config", cfg.to_str().unwrap()]), &h).unwrap();
        assert!(
            out.contains("1 pending")
                && out.contains("[undetermined]")
                && out.contains("when sammy appears"),
            "{out}"
        );
        // first back-off is 0 hours: due now
        let (out, _) = run("retry", &s(&["--config", cfg.to_str().unwrap()]), &h).unwrap();
        assert!(out.contains("1 due, 1 with a file present"), "{out}");
        // now backed off a day: nothing due, unless forced
        let (out, _) = run("retry", &s(&["--config", cfg.to_str().unwrap()]), &h).unwrap();
        assert!(out.contains("0 due"), "{out}");
        let (out, _) = run(
            "retry",
            &s(&["--config", cfg.to_str().unwrap(), "--all"]),
            &h,
        )
        .unwrap();
        assert!(out.contains("1 due"), "{out}");
        // Sammy shows up: what waited for him is due at once
        let (out, _) = run(
            "pending",
            &s(&["--config", cfg.to_str().unwrap(), "--wake", "sammy"]),
            &h,
        )
        .unwrap();
        assert!(out.contains("woke items waiting for: sammy"), "{out}");
        let (out, _) = run("retry", &s(&["--config", cfg.to_str().unwrap()]), &h).unwrap();
        assert!(out.contains("1 due"), "{out}");
    }
}
