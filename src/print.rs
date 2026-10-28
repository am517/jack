//! `--print` output.
use crate::config::{Config, Settings};
use crate::meta::Meta;
use std::fmt::Write as _;
use std::path::Path;
use crate::art::*;

/// `--print` output: (text, json entries).
#[derive(Default)]
pub(crate) struct Printed {
    pub(crate) text: String,
    pub(crate) json: Vec<serde_json::Value>,
}

pub(crate) fn print_meta(path: &Path, s: &Settings, cfg: &Config, meta: &Meta, p: &mut Printed) {
    let mut m = meta.clone();
    if let Some(u) = art_url(meta, s, cfg) {
        m.insert("art_url".into(), u);
    }
    if s.print_format.as_deref() == Some("json") {
        p.json.push(serde_json::json!({"file": path.to_string_lossy(), "metadata": m}));
    } else {
        let _ = writeln!(p.text, "{}", path.display());
        for (k, v) in &m {
            let _ = writeln!(p.text, "  {k}: {}", v.replace('\n', " "));
        }
        p.text.push('\n');
    }
}
