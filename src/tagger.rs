//! Builds and runs tag-writing commands (AtomicParsley / exiftool for MP4, mkvpropedit for MKV).
use crate::config::{TaggerCfg, has_ext};
use crate::meta::Meta;
use std::path::Path;
use std::process::Command;

fn kv(a: &mut Vec<String>, flag: &str, v: Option<&String>) {
    if let Some(v) = v.filter(|v| !v.is_empty()) {
        a.push(flag.into());
        a.push(v.clone());
    }
}

fn eq(a: &mut Vec<String>, tag: &str, v: Option<&String>) {
    if let Some(v) = v.filter(|v| !v.is_empty()) {
        a.push(format!("-{tag}={v}"));
    }
}

/// The argv to tag `file` with `m`.
pub fn command(file: &Path, m: &Meta, cfg: &TaggerCfg) -> Result<Vec<String>, String> {
    let f = file.to_string_lossy().into_owned();
    let ep = m.get("kind").is_some_and(|k| k == "episode");
    let title = if ep { m.get("episode_title").or(m.get("title")) } else { m.get("title") };
    let date = m.get("date").or(m.get("year"));
    let art = m
        .get("poster_file")
        .filter(|p| cfg.embed_artwork && Path::new(p).is_file());
    let desc = m.get("description");
    let mut a = Vec::new();
    if has_ext(file, &cfg.mkv_extensions) {
        a.extend([cfg.mkvpropedit.clone(), f]);
        if let Some(t) = title {
            a.extend(["--edit".into(), "info".into(), "--set".into(), format!("title={t}")]);
        }
        if let Some(p) = art {
            for s in ["--attachment-name", "cover.jpg", "--attachment-mime-type", "image/jpeg", "--add-attachment"] {
                a.push(s.into());
            }
            a.push(p.clone());
        }
    } else if has_ext(file, &cfg.mp4_extensions) {
        match cfg.mp4_tool.to_ascii_lowercase().as_str() {
            "atomicparsley" => {
                a.extend([cfg.atomicparsley.clone(), f, "--overWrite".into(), "--stik".into()]);
                a.push(if ep { "TV Show" } else { "Movie" }.into());
                kv(&mut a, "--title", title);
                kv(&mut a, "--year", date);
                let short = desc.map(|d| d.chars().take(cfg.short_description_max).collect::<String>());
                kv(&mut a, "--description", short.as_ref());
                kv(&mut a, "--longdesc", desc);
                kv(&mut a, "--genre", m.get("genre"));
                if ep {
                    kv(&mut a, "--TVShowName", m.get("show"));
                    kv(&mut a, "--artist", m.get("show"));
                    kv(&mut a, "--TVSeasonNum", m.get("season"));
                    kv(&mut a, "--TVEpisodeNum", m.get("episode"));
                    kv(&mut a, "--tracknum", m.get("episode"));
                }
                kv(&mut a, "--artwork", art);
            }
            "exiftool" => {
                a.extend([cfg.exiftool.clone(), "-overwrite_original".into()]);
                a.push(format!("-MediaType={}", if ep { "TV Show" } else { "Movie" }));
                eq(&mut a, "Title", title);
                eq(&mut a, "ContentCreateDate", date);
                eq(&mut a, "Description", desc);
                eq(&mut a, "LongDescription", desc);
                eq(&mut a, "Genre", m.get("genre"));
                if ep {
                    eq(&mut a, "TVShow", m.get("show"));
                    eq(&mut a, "TVSeason", m.get("season"));
                    eq(&mut a, "TVEpisode", m.get("episode"));
                }
                if let Some(p) = art {
                    a.push(format!("-CoverArt<={p}"));
                }
                a.push(f);
            }
            t => return Err(format!("tagger.mp4_tool: unknown tool '{t}' (use atomicparsley or exiftool)")),
        }
    } else {
        return Err(format!("{}: no tagger configured for this extension", file.display()));
    }
    Ok(a)
}

pub fn run(argv: &[String]) -> Result<(), String> {
    let out = Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| format!("{}: {e} (install it, e.g. `brew install atomicparsley exiftool mkvtoolnix`, or set its path under [tagger])", argv[0]))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("{} failed: {}", argv[0], String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Shell-quoted command line, for dry-run output.
pub fn show(argv: &[String]) -> String {
    argv.iter()
        .map(|s| {
            if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+".contains(c)) {
                s.clone()
            } else {
                format!("'{}'", s.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn cfg() -> TaggerCfg {
        config::load(Some(Path::new("config/jack.toml"))).unwrap().tagger
    }

    fn movie() -> Meta {
        [("kind", "movie"), ("title", "The Matrix"), ("year", "1999"), ("genre", "Sci-Fi"), ("description", "Neo.")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn episode() -> Meta {
        let mut m = movie();
        m.extend(
            [("kind", "episode"), ("show", "Saiki K"), ("season", "1"), ("episode", "2"), ("episode_title", "Ep")]
                .map(|(k, v)| (k.to_string(), v.to_string())),
        );
        m
    }

    #[test]
    fn atomicparsley_movie_and_episode() {
        let c = command(Path::new("a.mp4"), &movie(), &cfg()).unwrap();
        assert_eq!(&c[..5], ["AtomicParsley", "a.mp4", "--overWrite", "--stik", "Movie"]);
        assert!(show(&c).contains("--title 'The Matrix' --year 1999"));
        let c = show(&command(Path::new("a.m4v"), &episode(), &cfg()).unwrap());
        assert!(c.contains("--stik 'TV Show' --title Ep"), "{c}");
        assert!(c.contains("--TVShowName 'Saiki K'") && c.contains("--TVSeasonNum 1 --TVEpisodeNum 2"));
    }

    #[test]
    fn exiftool_and_mkv_and_unsupported() {
        let mut t = cfg();
        t.mp4_tool = "exiftool".into();
        let c = command(Path::new("a.mp4"), &episode(), &t).unwrap();
        assert!(c.contains(&"-TVShow=Saiki K".to_string()) && c.last().unwrap() == "a.mp4");
        let c = command(Path::new("b.mkv"), &movie(), &t).unwrap();
        assert_eq!(c, ["mkvpropedit", "b.mkv", "--edit", "info", "--set", "title=The Matrix"]);
        assert!(command(Path::new("c.avi"), &movie(), &t).is_err());
        t.mp4_tool = "nope".into();
        assert!(command(Path::new("a.mp4"), &movie(), &t).is_err());
    }

    fn have(tool: &str) -> bool {
        Command::new("which").arg(tool).output().is_ok_and(|o| o.status.success())
    }

    fn make(ext: &str) -> Option<(tempfile::TempDir, std::path::PathBuf)> {
        if !have("ffmpeg") {
            return None;
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(format!("t.{ext}"));
        let ok = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=16x16:d=0.2", "-c:v", "mpeg4"])
            .arg(&p)
            .status()
            .is_ok_and(|s| s.success());
        ok.then_some((d, p))
    }

    fn read_title(p: &Path) -> String {
        let o = Command::new("exiftool").args(["-s3", "-Title"]).arg(p).output().unwrap();
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    }

    /// Integration: runs only when ffmpeg + the tag tool + exiftool (reader) are installed.
    #[test]
    fn integration_real_tools() {
        let mut t = cfg();
        for (ext, tool, mp4) in [("mp4", "AtomicParsley", "atomicparsley"), ("mp4", "exiftool", "exiftool"), ("mkv", "mkvpropedit", "")] {
            if !have(tool) || !have("exiftool") {
                eprintln!("skip {tool}: not installed");
                continue;
            }
            let Some((_d, p)) = make(ext) else { continue };
            if !mp4.is_empty() {
                t.mp4_tool = mp4.into();
            }
            run(&command(&p, &movie(), &t).unwrap()).unwrap();
            assert_eq!(read_title(&p), "The Matrix", "{tool}");
        }
    }
}
