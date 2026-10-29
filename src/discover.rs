//! Capabilities: windups that answer `--capabilities`, and the sources their profiles name.
//! Anything not seen before is recorded and wakes what waits for it; a new source also wakes
//! everything waiting for "new_source".
use crate::caller::Caller;
use rusqlite::{Connection, params};

/// Registers answering windups; returns the capabilities that woke something.
pub fn discover(
    c: &Connection,
    cfg: &crate::config::Config,
    caller: &dyn Caller,
    extra: &[String],
) -> rusqlite::Result<Vec<String>> {
    for (name, cmd) in &cfg.windups {
        if let Some(p) = caller.capabilities(cmd) {
            let version = toml::from_str::<toml::Table>(&p)
                .ok()
                .and_then(|t| t.get("version").and_then(|v| v.as_str().map(String::from)))
                .unwrap_or_default();
            windups::registry::register(c, name, &version, &p)?;
        }
    }
    let mut caps: Vec<(String, &str)> = vec![];
    for (name, profile) in windups::registry::all(c)? {
        if let Ok(t) = toml::from_str::<toml::Table>(&profile) {
            for s in t
                .get("sources")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
            {
                caps.push((s.to_string(), "source"));
            }
        }
        caps.push((name, "windup"));
    }
    caps.extend(extra.iter().map(|e| (e.clone(), "named")));
    let mut woke = vec![];
    for (name, kind) in caps {
        let new = kind == "named"
            || c.execute(
                "INSERT OR IGNORE INTO capabilities(name, kind) VALUES (?1, ?2)",
                params![name, kind],
            )? > 0;
        if !new {
            continue;
        }
        let mut n = windups::returns::wake(c, &name)?;
        if kind == "source" {
            n += windups::returns::wake(c, "new_source")?;
        }
        if n > 0 {
            woke.push(name);
        }
    }
    Ok(woke)
}
