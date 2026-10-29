//! Daniel, the database windup. Takes one entry per file (`windups::handoff::Handoff`), asks Larry
//! for the file's hash and fingerprint and Ruth for the work reference, stores it all, then tells
//! Walter. Windups hand entries to Daniel; only Daniel writes them.
pub mod config;
pub mod store;
pub mod walter;

pub use store::StoreCfg;
use windups::handoff::{Daniel, Handoff};

pub const PROFILE: &str = include_str!("../PROFILE.toml");

pub struct Store<'a> {
    pub store: &'a StoreCfg,
    pub larry: larry::Config,
    pub config: config::Config,
}

impl<'a> Store<'a> {
    /// With Larry's and Daniel's user configs from ~/.config/windups/<name>/config.toml.
    pub fn new(store: &'a StoreCfg) -> Self {
        let user = |n: &str| {
            std::env::var_os("HOME")
                .map(|h| std::path::Path::new(&h).join(format!(".config/windups/{n}/config.toml")))
        };
        Store {
            store,
            larry: larry::Config::load(user("larry").as_deref()).unwrap_or_default(),
            config: config::Config::load(user("daniel").as_deref()).unwrap_or_default(),
        }
    }
}

impl Daniel for Store<'_> {
    fn accept(&self, h: &Handoff) -> Result<String, String> {
        let mut entry = h.entry.clone();
        let fp = larry::fingerprint::NoFingerprint;
        entry.extend(larry::describe(&h.file, &self.larry, &fp));
        let mut out = store::save(
            &h.file,
            &entry,
            &h.candidates,
            &h.conflicts,
            self.store,
            &fp,
            &self.larry.fingerprint,
        )?;
        let w = &self.config.walter;
        if w.enabled && out.starts_with("database:") {
            if let Some(n) = store::db_path(self.store)
                .ok()
                .and_then(|db| walter::notify(&w.command, &db))
            {
                out += &format!("; {n}");
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_handoff_with_larrys_hash_and_ruths_reference() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.mkv");
        std::fs::write(&f, "content").unwrap();
        let cfg = StoreCfg {
            enabled: true,
            database: d.path().join("m.db").to_string_lossy().into(),
            fallback_suffix: ".json".into(),
            busy_timeout_secs: 1,
        };
        let mut dan = Store::new(&cfg);
        dan.config.walter.enabled = false;
        let entry: windups::handoff::Fields = [
            ("kind", "movie"),
            ("title", "Heat"),
            ("year", "1995"),
            ("lifecycle", "unverified"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let r = dan
            .accept(&Handoff {
                file: f,
                entry,
                candidates: vec![],
                conflicts: vec![],
                from: "jack".into(),
            })
            .unwrap();
        assert!(r.starts_with("database: media #1"), "{r}");
        let c = windups::db::open(&d.path().join("m.db")).unwrap();
        let (hash, prov): (String, bool) = c
            .query_row(
                "SELECT f.hash, w.provisional FROM files f JOIN media m ON m.id = f.media_id JOIN works w ON w.id = m.work_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(hash.starts_with("b3"), "{hash}");
        assert!(prov, "no original title or language yet");
        assert!(PROFILE.contains("name = \"daniel\""));
    }
}
