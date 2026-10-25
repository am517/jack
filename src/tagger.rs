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

/// (tag name, value) for every configured id present in `m`.
fn ids<'a>(m: &'a Meta, names: &'a std::collections::BTreeMap<String, String>) -> Vec<(&'a str, &'a str)> {
    names
        .iter()
        .filter_map(|(k, n)| m.get(k).filter(|v| !v.is_empty()).map(|v| (n.as_str(), v.as_str())))
        .collect()
}

fn rdns(a: &mut Vec<String>, m: &Meta, cfg: &TaggerCfg) {
    for (n, v) in ids(m, &cfg.mp4_id_tags) {
        a.extend(["--rDNSatom".into(), v.into(), format!("name={n}"), format!("domain={}", cfg.id_domain)]);
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Matroska tags XML: `existing` (from mkvextract, may be empty) with our id simple tags
/// updated or inserted in the global tag. All other tags and targets are kept as-is.
/// None if `m` has no ids.
pub fn mkv_tags_xml(m: &Meta, cfg: &TaggerCfg, existing: &str) -> Option<String> {
    let v = ids(m, &cfg.mkv_id_tags);
    if v.is_empty() {
        return None;
    }
    let mut doc = if existing.contains("</Tags>") { existing.to_string() } else { "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Tags>\n</Tags>\n".into() };
    // global tag = a <Tag> whose <Targets> names no UID (track/edition/chapter/attachment)
    let tag_re = regex::Regex::new(r"(?s)<Tag>.*?</Tag>").unwrap();
    let global = tag_re.find_iter(&doc).find(|t| {
        let s = t.as_str();
        let tg = s.find("<Targets").map_or("", |i| &s[i..s[i..].find("</Targets>").map_or_else(|| s[i..].find("/>").map_or(s.len(), |j| i + j), |j| i + j)]);
        !tg.contains("UID>")
    });
    let (start, mut tag) = match global {
        Some(t) => (Some(t.range()), t.as_str().to_string()),
        None => (None, "<Tag><Targets/></Tag>".to_string()),
    };
    for (n, s) in v {
        let re = regex::Regex::new(&format!(r"(?s)(<Simple>\s*<Name>{}</Name>\s*<String>)[^<]*(</String>)", regex::escape(&xml(n)))).unwrap();
        if re.is_match(&tag) {
            tag = re.replace(&tag, |c: &regex::Captures| format!("{}{}{}", &c[1], xml(s), &c[2])).into_owned();
        } else {
            let i = tag.rfind("</Tag>").unwrap();
            tag.insert_str(i, &format!("<Simple><Name>{}</Name><String>{}</String></Simple>", xml(n), xml(s)));
        }
    }
    match start {
        Some(r) => doc.replace_range(r, &tag),
        None => {
            let i = doc.rfind("</Tags>").unwrap();
            doc.insert_str(i, &tag);
        }
    }
    Some(doc)
}

/// Extra commands to run after `command` (ids via AtomicParsley when exiftool is the MP4 writer,
/// since exiftool can't write iTunes freeform atoms).
pub fn extra_commands(file: &Path, m: &Meta, cfg: &TaggerCfg) -> Vec<Vec<String>> {
    let mut a = vec![cfg.atomicparsley.clone(), file.to_string_lossy().into_owned(), "--overWrite".into()];
    rdns(&mut a, m, cfg);
    let exif = cfg.mp4_tool.eq_ignore_ascii_case("exiftool");
    if exif && a.len() > 3 && has_ext(file, &cfg.mp4_extensions) { vec![a] } else { vec![] }
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
        a.extend([cfg.mkvpropedit.clone(), f.clone()]);
        if let Some(t) = title {
            a.extend(["--edit".into(), "info".into(), "--set".into(), format!("title={t}")]);
        }
        if let Some(p) = art {
            for s in ["--attachment-name", "cover.jpg", "--attachment-mime-type", "image/jpeg", "--add-attachment"] {
                a.push(s.into());
            }
            a.push(p.clone());
        }
        let existing = output(&[&cfg.mkvextract, &f, "tags", "-"]).unwrap_or_default();
        if let Some(x) = mkv_tags_xml(m, cfg, &existing) {
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let tp = std::env::temp_dir().join(format!("jack-{}-{name}.tags.xml", std::process::id()));
            std::fs::write(&tp, x).map_err(|e| format!("{}: {e}", tp.display()))?;
            a.extend(["--tags".into(), format!("all:{}", tp.display())]);
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
                rdns(&mut a, m, cfg);
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

fn output(argv: &[&str]) -> Option<String> {
    let o = Command::new(argv[0]).args(&argv[1..]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Ids previously written into `file` (empty if none, or the reader tool is missing).
pub fn read_ids(file: &Path, cfg: &TaggerCfg) -> Meta {
    let f = file.to_string_lossy();
    let mut found: Vec<(String, String)> = vec![];
    let names = if has_ext(file, &cfg.mkv_extensions) {
        if let Some(x) = output(&[&cfg.mkvextract, &f, "tags", "-"]) {
            let re = regex::Regex::new(r"(?s)<Simple>\s*<Name>([^<]*)</Name>\s*<String>([^<]*)</String>").unwrap();
            found = re.captures_iter(&x).map(|c| (c[1].to_string(), c[2].replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))).collect();
        }
        &cfg.mkv_id_tags
    } else if has_ext(file, &cfg.mp4_extensions) {
        if cfg.mp4_tool.eq_ignore_ascii_case("exiftool") {
            if let Some(j) = output(&[&cfg.exiftool, "-j", "-iTunes:all", &f]) {
                if let Ok(serde_json::Value::Array(v)) = serde_json::from_str(&j) {
                    if let Some(serde_json::Value::Object(o)) = v.first() {
                        found = o.iter().map(|(k, v)| (k.clone(), v.as_str().map_or_else(|| v.to_string(), String::from))).collect();
                    }
                }
            }
        } else if let Some(x) = output(&[&cfg.atomicparsley, &f, "-t"]) {
            let pre = format!("Atom \"----\" [{};", cfg.id_domain);
            for l in x.lines() {
                if let Some((n, v)) = l.strip_prefix(&pre).and_then(|r| r.split_once("] contains: ")) {
                    found.push((n.to_string(), v.trim().to_string()));
                }
            }
        }
        &cfg.mp4_id_tags
    } else {
        return Meta::new();
    };
    names
        .iter()
        .filter_map(|(k, n)| found.iter().find(|(fname, _)| fname == n).map(|(_, v)| (k.clone(), v.clone())))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

pub fn run(argv: &[String]) -> Result<(), String> {
    let out = Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| format!("{}: {e} (install it, e.g. `brew install atomicparsley exiftool mkvtoolnix`, or set its path under [tagger])", argv[0]))?;
    for t in argv.iter().filter_map(|x| x.strip_prefix("all:")).filter(|x| x.ends_with(".tags.xml")) {
        let _ = std::fs::remove_file(t); // temp tags file from `command`
    }
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

    fn ids_meta(mut m: Meta) -> Meta {
        for (k, v) in [("imdb_id", "tt0133093"), ("tmdb_id", "603"), ("tmdb_episode_id", "77"), ("thetvdb_series_id", "7")] {
            m.insert(k.into(), v.into());
        }
        m
    }

    #[test]
    fn id_args_and_xml() {
        let c = show(&command(Path::new("a.mp4"), &ids_meta(movie()), &cfg()).unwrap());
        assert!(c.contains("--rDNSatom tt0133093 name=IMDB_ID domain=com.apple.iTunes"), "{c}");
        assert!(c.contains("--rDNSatom 77 name=TMDB_EPISODE_ID"));
        let x = mkv_tags_xml(&ids_meta(movie()), &cfg(), "").unwrap();
        assert!(x.contains("<Name>IMDB</Name><String>tt0133093</String>") && x.contains("<Name>TVDB_SERIES</Name>"));
        assert!(mkv_tags_xml(&movie(), &cfg(), "").is_none());
        // merge: keeps track tags + other global tags, updates ours in place, idempotent
        let old = "<Tags><Tag><Targets><TrackUID>5</TrackUID></Targets><Simple><Name>IMDB</Name><String>trk</String></Simple></Tag>\
                   <Tag><Targets /><Simple><Name>ARTIST</Name><String>A &amp; B</String></Simple><Simple><Name>IMDB</Name><String>old</String></Simple></Tag></Tags>";
        let x = mkv_tags_xml(&ids_meta(movie()), &cfg(), old).unwrap();
        assert!(x.contains("<TrackUID>5</TrackUID></Targets><Simple><Name>IMDB</Name><String>trk</String>"), "{x}");
        assert!(x.contains("<Name>ARTIST</Name><String>A &amp; B</String>") && x.contains("<Name>IMDB</Name><String>tt0133093</String>") && !x.contains(">old<"));
        assert_eq!(mkv_tags_xml(&ids_meta(movie()), &cfg(), &x).unwrap(), x);
        let mut t = cfg();
        t.mp4_tool = "exiftool".into();
        assert_eq!(extra_commands(Path::new("a.mp4"), &ids_meta(movie()), &t).len(), 1);
        assert!(extra_commands(Path::new("a.mp4"), &movie(), &t).is_empty());
        assert!(extra_commands(Path::new("a.mp4"), &ids_meta(movie()), &cfg()).is_empty());
    }

    /// Integration: write ids into real ffmpeg-made MP4/MKV files with each writer, then read them back.
    #[test]
    fn integration_ids_roundtrip() {
        let mut t = cfg();
        for (ext, mp4, tools) in [("mp4", "atomicparsley", &["AtomicParsley"][..]), ("mp4", "exiftool", &["exiftool", "AtomicParsley"]), ("mkv", "", &["mkvpropedit", "mkvextract"])] {
            if !tools.iter().all(|x| have(x)) {
                eprintln!("skip ids {ext}/{mp4}: tools missing");
                continue;
            }
            let Some((_d, p)) = make(ext) else { continue };
            if !mp4.is_empty() {
                t.mp4_tool = mp4.into();
            }
            let m = ids_meta(movie());
            assert!(read_ids(&p, &t).is_empty());
            run(&command(&p, &m, &t).unwrap()).unwrap();
            for c in extra_commands(&p, &m, &t) {
                run(&c).unwrap();
            }
            let r = read_ids(&p, &t);
            assert_eq!(r.get("imdb_id").map(String::as_str), Some("tt0133093"), "{ext}/{mp4}: {r:?}");
            assert_eq!(r.get("tmdb_id").map(String::as_str), Some("603"));
            assert_eq!(r.get("tmdb_episode_id").map(String::as_str), Some("77"));
            assert_eq!(r.get("thetvdb_series_id").map(String::as_str), Some("7"));
            assert_eq!(r.len(), 4);
        }
    }

    /// Integration: pre-existing MKV global tags survive id writes; re-running is idempotent.
    #[test]
    fn integration_mkv_tags_merge() {
        if !["mkvpropedit", "mkvextract"].iter().all(|x| have(x)) {
            return eprintln!("skip mkv merge: tools missing");
        }
        let Some((d, p)) = make("mkv") else { return };
        let t = cfg();
        let pre = d.path().join("pre.xml");
        std::fs::write(&pre, "<Tags><Tag><Targets/><Simple><Name>ARTIST</Name><String>Someone</String></Simple><Simple><Name>MY_CUSTOM</Name><String>keep me</String></Simple></Tag></Tags>").unwrap();
        run(&[t.mkvpropedit.clone(), p.to_string_lossy().into(), "--tags".into(), format!("global:{}", pre.display())]).unwrap();
        let tags = || output(&[&t.mkvextract, &p.to_string_lossy(), "tags", "-"]).unwrap();
        let m = ids_meta(movie());
        run(&command(&p, &m, &t).unwrap()).unwrap();
        let once = tags();
        assert!(once.contains("Someone") && once.contains("keep me") && once.contains("ENCODER"), "{once}");
        assert_eq!(read_ids(&p, &t).len(), 4);
        run(&command(&p, &m, &t).unwrap()).unwrap();
        assert_eq!(tags(), once);
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
