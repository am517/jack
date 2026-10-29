//! Second label reader: a source URL in the file's own tags (comment, purl, description...).
use super::{Label, OnlineCfg, urls};
use std::path::Path;

/// Tag values from ffprobe; empty if it's missing or the file has none.
fn tags(path: &Path, cfg: &OnlineCfg) -> Vec<(String, String)> {
    let Ok(o) = std::process::Command::new(&cfg.ffprobe)
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()
    else {
        return vec![];
    };
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
    v["format"]["tags"]
        .as_object()
        .map(|t| {
            t.iter()
                .map(|(k, v)| (k.to_lowercase(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn from_tags(t: &[(String, String)], cfg: &OnlineCfg) -> Option<Label> {
    cfg.embedded_tags.iter().find_map(|want| {
        t.iter()
            .filter(|(k, _)| k == want)
            .find_map(|(_, v)| urls::parse(v))
            .map(|(site, id)| Label {
                site,
                id,
                reader: format!("embedded {want}"),
                ..Default::default()
            })
    })
}

pub fn read(path: &Path, cfg: &OnlineCfg) -> Option<Label> {
    from_tags(&tags(path, cfg), cfg)
}

#[cfg(test)]
mod tests {
    #[test]
    fn url_in_configured_tag() {
        let c = super::OnlineCfg::default();
        let t = vec![
            ("title".into(), "https://youtu.be/aaaaaaaaaaa".into()),
            (
                "purl".into(),
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
            ),
        ];
        let l = super::from_tags(&t, &c).unwrap();
        assert_eq!(
            (l.id.as_str(), l.reader.as_str()),
            ("dQw4w9WgXcQ", "embedded purl")
        );
        assert!(super::from_tags(&t[..1], &c).is_none());
    }
}
