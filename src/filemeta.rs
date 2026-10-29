//! The file's own embedded metadata (read first, like a person reading the label).
//! Always kept as source "embedded"; never overrides a source, only compared (see conflicts).
use crate::meta::Meta;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(serde::Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct EmbeddedCfg {
    pub enabled: bool,
    /// Lowercased container tag -> meta key; the first tag found per key wins.
    pub tags: BTreeMap<String, String>,
}

/// Container tags (lowercased, via ffprobe) -> meta keys. Tag map lives in config ([embedded]).
pub fn from_tags(tags: &[(String, String)], map: &BTreeMap<String, String>) -> Meta {
    let mut m = Meta::new();
    for (tag, key) in map {
        if let Some((_, v)) = tags.iter().find(|(k, v)| k == tag && !v.trim().is_empty()) {
            m.entry(key.clone()).or_insert_with(|| v.trim().to_string());
        }
    }
    if let Some(y) = m
        .get("date")
        .and_then(|d| d.get(..4))
        .filter(|y| y.bytes().all(|b| b.is_ascii_digit()))
        .map(String::from)
    {
        m.entry("year".into()).or_insert(y);
    }
    m
}

/// Empty when ffprobe is missing or the file carries nothing.
pub fn read(path: &Path, ffprobe: &str, map: &BTreeMap<String, String>) -> Meta {
    let Ok(o) = std::process::Command::new(ffprobe)
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()
    else {
        return Meta::new();
    };
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_default();
    let tags: Vec<(String, String)> = v["format"]["tags"]
        .as_object()
        .map(|t| {
            t.iter()
                .map(|(k, v)| (k.to_lowercase(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();
    from_tags(&tags, map)
}

#[cfg(test)]
mod tests {
    #[test]
    fn maps_configured_tags() {
        let cfg = crate::config::load(Some(std::path::Path::new("config/jack.toml"))).unwrap();
        let t = vec![
            ("title".into(), "Pilot".into()),
            ("show".into(), "Breaking Bad".into()),
            ("date".into(), "2008-01-20".into()),
            ("encoder".into(), "x".into()),
        ];
        let m = super::from_tags(&t, &cfg.embedded.tags);
        assert_eq!(
            (m["title"].as_str(), m["show"].as_str(), m["year"].as_str()),
            ("Pilot", "Breaking Bad", "2008")
        );
        assert!(!m.contains_key("encoder"));
    }
}
