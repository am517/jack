//! `--ask-art`: download every art candidate into a folder, list them, let the user pick one.
use crate::meta::Meta;
use crate::providers::Http;
use std::cell::RefCell;
use std::fmt::Write as _;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Prompt I/O: stdin + stderr normally; tests inject input and capture the log.
pub struct Ui {
    pub input: Box<dyn BufRead>,
    pub tty: bool,
    /// None: write to stderr; Some: captured (tests).
    pub log: Option<String>,
}

impl Ui {
    fn say(&mut self, s: &str) {
        match &mut self.log {
            Some(l) => l.push_str(s),
            None => eprint!("{s}"),
        }
    }
}

thread_local! {
    pub static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn with_ui<R>(f: impl FnOnce(&mut Ui) -> R) -> R {
    UI.with(|u| {
        let mut u = u.borrow_mut();
        let ui = u.get_or_insert_with(|| {
            use std::io::IsTerminal;
            Ui {
                input: Box::new(std::io::BufReader::new(std::io::stdin())),
                tty: std::io::stdin().is_terminal(),
                log: None,
            }
        });
        f(ui)
    })
}

pub use crate::providers::Img as Cand;

fn key_of(kind: &str) -> &'static str {
    match kind {
        "backdrop" => "backdrop_url",
        "still" => "still_url",
        "season" => "season_poster_url",
        _ => "poster_url",
    }
}

/// Candidates: the found art URLs plus `extra` (full image lists), ordered by style key (as art_url),
/// then provider preference for that key, then language (primary, original, untagged, other), then
/// resolution; deduped by file name, capped at `max`.
pub fn candidates(
    found: &[(String, Meta)],
    extra: Vec<Cand>,
    keys: &[&str],
    prefs: impl Fn(&str) -> Vec<String>,
    max: usize,
) -> Vec<Cand> {
    let mut all: Vec<Cand> = vec![];
    for k in keys {
        for (p, m) in found {
            if let Some(u) = m.get(*k).filter(|u| !u.is_empty()) {
                let kind = ["backdrop", "still", "season", "poster"]
                    .into_iter()
                    .find(|x| key_of(x) == *k)
                    .unwrap();
                all.push(Cand {
                    provider: p.clone(),
                    kind,
                    url: u.clone(),
                    lang: String::new(),
                    lrank: 2,
                    w: 0,
                    h: 0,
                });
            }
        }
    }
    all.extend(extra.into_iter().filter(|c| keys.contains(&key_of(c.kind))));
    let rank = |c: &Cand| {
        let k = key_of(c.kind);
        let pr = prefs(k)
            .iter()
            .position(|p| *p == c.provider)
            .unwrap_or(usize::MAX);
        (
            keys.iter().position(|x| *x == k),
            pr,
            c.lrank,
            std::cmp::Reverse(c.w * c.h),
        )
    };
    all.sort_by_key(rank);
    let name = |u: &str| u.rsplit('/').next().unwrap_or(u).to_string();
    let mut v: Vec<Cand> = vec![];
    for c in all {
        if !v.iter().any(|x| name(&x.url) == name(&c.url)) {
            v.push(c);
        }
    }
    v.truncate(max.max(1));
    v
}

fn ext(url: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    match last.rsplit_once('.') {
        Some((_, e))
            if !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            e.to_ascii_lowercase()
        }
        _ => "jpg".into(),
    }
}

pub struct Opts<'a> {
    pub open_cmd: Option<&'a str>,
    pub keep: bool,
}

/// Downloads, lists, asks. Returns the chosen image copied to `poster`, or None (skipped / nothing).
pub fn ask(
    media: &Path,
    poster: &Path,
    cands: &[Cand],
    http: &dyn Http,
    o: &Opts,
    out: &mut String,
) -> Result<Option<PathBuf>, String> {
    if cands.is_empty() {
        let _ = writeln!(out, "    ask-art: no candidates");
        return Ok(None);
    }
    let stem = media
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = media.with_file_name(stem);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut got: Vec<(PathBuf, &Cand)> = vec![];
    for c in cands {
        match http.bytes(&c.url) {
            Ok(b) => {
                let f = dir.join(format!("{:02}.{}", got.len() + 1, ext(&c.url)));
                std::fs::write(&f, &b).map_err(|e| format!("{}: {e}", f.display()))?;
                got.push((f, c));
            }
            Err(e) => {
                let _ = writeln!(out, "    warning: artwork: {e}");
            }
        }
    }
    let mut list = format!(
        "{}: {} image(s) in {}\n",
        media.display(),
        got.len(),
        dir.display()
    );
    for (i, (f, c)) in got.iter().enumerate() {
        let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        let dim = if c.w > 0 {
            format!("{}x{}", c.w, c.h)
        } else {
            format!("{size} bytes")
        };
        let lang = if c.lang.is_empty() { "-" } else { &c.lang };
        let _ = writeln!(
            list,
            "  {:>2}. {:<8} {:<8} {:<4} {:<12} {}",
            i + 1,
            c.provider,
            c.kind,
            lang,
            dim,
            f.file_name().unwrap().to_string_lossy()
        );
    }
    let pick = with_ui(|ui| {
        ui.say(&list);
        if got.is_empty() {
            return None;
        }
        if let Some(cmd) = o.open_cmd.filter(|_| ui.log.is_none() && ui.tty) {
            let _ = std::process::Command::new(cmd).arg(&dir).status();
        }
        if !ui.tty {
            ui.say("warning: stdin is not a terminal; using #1\n");
            return Some(0);
        }
        loop {
            ui.say(&format!("Choose 1-{} (Enter = 1, s = skip): ", got.len()));
            let mut l = String::new();
            if ui.input.read_line(&mut l).unwrap_or(0) == 0 {
                return Some(0); // EOF
            }
            match l.trim() {
                "" => return Some(0),
                "s" | "S" => return None,
                n => match n.parse::<usize>() {
                    Ok(i) if (1..=got.len()).contains(&i) => return Some(i - 1),
                    _ => ui.say(&format!("invalid choice '{n}'\n")),
                },
            }
        }
    });
    let chosen = match pick {
        Some(i) => {
            std::fs::copy(&got[i].0, poster).map_err(|e| format!("{}: {e}", poster.display()))?;
            let _ = writeln!(
                out,
                "    ask-art: chose #{} ({} {})",
                i + 1,
                got[i].1.provider,
                got[i].1.kind
            );
            Some(poster.to_path_buf())
        }
        None => {
            let _ = writeln!(out, "    ask-art: skipped");
            None
        }
    };
    if o.keep {
        let _ = writeln!(out, "    ask-art: kept {}", dir.display());
    } else {
        for (f, _) in &got {
            let _ = std::fs::remove_file(f);
        }
        let _ = std::fs::remove_dir(&dir); // only if now empty
    }
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_dedupe_cap() {
        let c = |p: &str, kind: &'static str, url: &str, lrank: u8, w: u64| Cand {
            provider: p.into(),
            kind,
            url: url.into(),
            lang: String::new(),
            lrank,
            w,
            h: w,
        };
        let found = vec![
            (
                "tmdb".to_string(),
                [("poster_url".to_string(), "http://i/w780/a.jpg".to_string())]
                    .into_iter()
                    .collect::<Meta>(),
            ),
            (
                "omdb".to_string(),
                [("poster_url".to_string(), "http://o/x.jpg".to_string())]
                    .into_iter()
                    .collect(),
            ),
        ];
        let extra = vec![
            c("tmdb", "poster", "http://i/original/lo.jpg", 0, 100),
            c("tmdb", "poster", "http://i/original/a.jpg", 0, 2000),
            c("thetvdb", "poster", "http://v/t.jpg", 0, 9000),
            c("tmdb", "poster", "http://i/original/ja.jpg", 1, 9000),
            c("tmdb", "backdrop", "http://i/original/b.jpg", 0, 9000),
            c("tmdb", "still", "http://i/original/s.jpg", 0, 9000),
        ];
        let prefs = |_: &str| {
            vec![
                "tmdb".to_string(),
                "thetvdb".to_string(),
                "omdb".to_string(),
            ]
        };
        let v = candidates(
            &found,
            extra.clone(),
            &["poster_url", "backdrop_url"],
            prefs,
            20,
        );
        let urls: Vec<_> = v
            .iter()
            .map(|c| c.url.rsplit('/').next().unwrap())
            .collect();
        // style key, then provider preference, then language, then resolution; a.jpg deduped (bigger kept); still not in style
        assert_eq!(
            urls,
            vec!["a.jpg", "lo.jpg", "ja.jpg", "t.jpg", "x.jpg", "b.jpg"]
        );
        assert_eq!(v[0].w, 2000);
        assert_eq!(
            candidates(&found, extra, &["poster_url"], prefs, 2).len(),
            2
        );
    }
}
