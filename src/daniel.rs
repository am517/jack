//! The handoff: Jack only fetches and verifies; one entry per file goes to Daniel, the database
//! windup. Until Daniel has his own crate, `LocalDaniel` is a stub that wraps the store code
//! (store.rs, Daniel's to take) and asks Larry for the file's hash and fingerprint, as Daniel will.
use crate::meta::Meta;
use crate::store::StoreCfg;
use std::path::Path;
use windups::handoff::{Daniel, Handoff};

pub struct LocalDaniel<'a> {
    pub store: &'a StoreCfg,
    pub larry: larry::Config,
}

impl Daniel for LocalDaniel<'_> {
    fn accept(&self, h: &Handoff) -> Result<String, String> {
        let mut entry = h.entry.clone();
        entry.extend(larry::describe(
            &h.file,
            &self.larry,
            &larry::fingerprint::NoFingerprint,
        ));
        crate::store::save(
            &h.file,
            &entry,
            &h.candidates,
            self.store,
            &larry::fingerprint::NoFingerprint,
            &self.larry.fingerprint,
        )
    }
}

fn larry_config() -> larry::Config {
    let p =
        std::env::var_os("HOME").map(|h| Path::new(&h).join(".config/windups/larry/config.toml"));
    larry::Config::load(p.as_deref()).unwrap_or_default()
}

/// Hands one entry for `path` to Daniel.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    store: &StoreCfg,
) -> Result<String, String> {
    let d = LocalDaniel {
        store,
        larry: larry_config(),
    };
    d.accept(&Handoff {
        file: path.to_path_buf(),
        entry: entry.clone(),
        candidates: found.to_vec(),
        from: "jack".into(),
    })
}
