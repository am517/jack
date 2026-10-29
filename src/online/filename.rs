//! Last label reader: a YouTube id in the filename, e.g. "Title [dQw4w9WgXcQ].mp4".
use super::{Label, OnlineCfg};
use std::path::Path;

pub fn read(path: &Path, cfg: &OnlineCfg) -> Option<Label> {
    let name = path.file_name()?.to_string_lossy();
    let re = regex::Regex::new(&cfg.filename_id_pattern).ok()?;
    let id = re.captures(&name)?.get(1)?.as_str().to_string();
    Some(Label { site: "youtube".into(), id, reader: "filename".into(), ..Default::default() })
}

#[cfg(test)]
mod tests {
    #[test]
    fn id_in_brackets_only() {
        let c = super::OnlineCfg::default();
        let r = |n: &str| super::read(std::path::Path::new(n), &c).map(|l| l.id);
        assert_eq!(r("/v/Big Talk [dQw4w9WgXcQ].mp4").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(r("/v/Breaking.Bad.S01E01.mkv"), None);
    }
}
