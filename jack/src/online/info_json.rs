//! First label reader: the yt-dlp .info.json beside the file. Works for YouTube and X alike.
use super::{Label, OnlineCfg, put_common};
use crate::meta::Meta;
use serde_json::Value;
use std::path::Path;

pub fn sidecar(path: &Path, cfg: &OnlineCfg) -> std::path::PathBuf {
    let stem = path.with_extension("");
    std::path::PathBuf::from(format!("{}{}", stem.display(), cfg.info_json_suffix))
}

pub fn parse(v: &Value) -> Option<Label> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let site = match s("extractor_key").to_lowercase().as_str() {
        "youtube" => "youtube",
        "twitter" | "x" => "x",
        _ => return None,
    };
    let mut m = Meta::new();
    let tags: Vec<String> = v["tags"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let channel = [s("channel"), s("uploader")]
        .into_iter()
        .find(|c| !c.is_empty())
        .unwrap_or_default();
    let chid = [s("channel_id"), s("uploader_id")]
        .into_iter()
        .find(|c| !c.is_empty())
        .unwrap_or_default();
    put_common(
        &mut m,
        &s("title"),
        &channel,
        &chid,
        &s("description"),
        &s("upload_date"),
        &tags,
        &s("thumbnail"),
    );
    let id = s("id");
    if !s("webpage_url").is_empty() {
        m.insert("video_url".into(), s("webpage_url"));
    }
    m.insert(format!("{site}_video_id"), id.clone());
    if let Some(d) = v["duration"].as_f64() {
        m.insert("runtime".into(), ((d / 60.0).round() as i64).to_string());
    }
    Some(Label {
        site: site.into(),
        id,
        info: m,
        reader: "info.json".into(),
    })
}

pub fn read(path: &Path, cfg: &OnlineCfg) -> Option<Label> {
    let t = std::fs::read_to_string(sidecar(path, cfg)).ok()?;
    parse(&serde_json::from_str(&t).ok()?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    #[test]
    fn reads_sidecar_youtube_and_x() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Talk [dQw4w9WgXcQ].mp4");
        std::fs::write(d.path().join("Talk [dQw4w9WgXcQ].info.json"), json!({"extractor_key":"Youtube","id":"dQw4w9WgXcQ","title":"Talk","channel":"Chan","channel_id":"UC9","upload_date":"20200102","tags":["x"],"duration":600,"webpage_url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}).to_string()).unwrap();
        let l = super::read(&f, &super::OnlineCfg::default()).unwrap();
        assert_eq!(
            (
                l.site.as_str(),
                l.info["channel"].as_str(),
                l.info["date"].as_str(),
                l.info["runtime"].as_str()
            ),
            ("youtube", "Chan", "2020-01-02", "10")
        );
        let x = super::parse(
            &json!({"extractor_key":"Twitter","id":"42","title":"t","uploader":"Someone"}),
        )
        .unwrap();
        assert_eq!(
            (
                x.site.as_str(),
                x.info["x_video_id"].as_str(),
                x.info["channel"].as_str()
            ),
            ("x", "42", "Someone")
        );
    }
}
