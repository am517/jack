//! YouTube videos.list by id: 1 quota unit (a search costs 100).
use super::put_common;
use crate::meta::Meta;
use crate::providers::{Http, enc};
use crate::sources::youtube::YoutubeCfg;

pub fn lookup(id: &str, cfg: &YoutubeCfg, http: &dyn Http) -> Result<Option<Meta>, String> {
    if cfg.api_key.is_empty() {
        return Ok(None);
    }
    let url = format!("{}/videos?part=snippet,contentDetails&id={}&key={}", cfg.base_url, enc(id), enc(&cfg.api_key));
    let r = http.get(&url, &[]).map_err(|e| e.replace(&cfg.api_key, "****"))?;
    let sn = &r["items"][0]["snippet"];
    if sn.is_null() {
        return Ok(None);
    }
    let s = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
    let th = &sn["thumbnails"];
    let thumb = ["maxres", "standard", "high", "medium", "default"].iter().map(|k| s(&th[*k]["url"])).find(|u| !u.is_empty()).unwrap_or_default();
    let tags: Vec<String> = sn["tags"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default();
    let mut m = Meta::new();
    put_common(&mut m, &s(&sn["title"]), &s(&sn["channelTitle"]), &s(&sn["channelId"]), &s(&sn["description"]), &s(&sn["publishedAt"]), &tags, &thumb);
    m.insert("youtube_video_id".into(), id.into());
    m.insert("video_url".into(), format!("{}{id}", cfg.watch_url));
    Ok(Some(m))
}
