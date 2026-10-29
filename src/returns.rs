//! When an unfinished entry comes back (from [retry]): after a back-off time, or as soon as what
//! it waits for appears. Written into the entry's meta; the store schedules it in `returns`.
use crate::meta::Meta;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct RetryCfg {
    /// Hours until the next return, by attempt (the last value repeats).
    pub backoff_hours: Vec<i64>,
    /// Capability a stage waits for (e.g. not_identified = "sammy"); besides the time.
    pub wait_for: BTreeMap<String, String>,
}

impl Default for RetryCfg {
    fn default() -> Self {
        Self {
            backoff_hours: vec![1, 24, 168, 720],
            wait_for: BTreeMap::from([("not_identified".into(), "sammy".into())]),
        }
    }
}

pub const WAITING_FOR: &str = "return_waiting_for";
pub const BACKOFF: &str = "return_backoff_hours";

/// Marks the entry's return plan in its meta (nothing for complete entries).
pub fn plan(meta: &mut Meta, cfg: &RetryCfg) {
    let stage = meta.get("lifecycle").cloned().unwrap_or_default();
    if stage == "complete" {
        return;
    }
    if let Some(w) = cfg.wait_for.get(&stage).filter(|w| !w.is_empty()) {
        meta.insert(WAITING_FOR.into(), w.clone());
    }
    let b: Vec<String> = cfg.backoff_hours.iter().map(|h| h.to_string()).collect();
    meta.insert(BACKOFF.into(), b.join(","));
}

/// Back-off hours from meta.
pub fn backoff(meta: &Meta) -> Vec<i64> {
    meta.get(BACKOFF)
        .map(|s| s.split(',').filter_map(|h| h.trim().parse().ok()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plans_from_config() {
        let t: toml::Table = toml::from_str(crate::config::DEFAULT_CONFIG).unwrap();
        let cfg: RetryCfg = t["retry"].clone().try_into().unwrap();
        let mut m: Meta = [("lifecycle".to_string(), "not_identified".to_string())].into();
        plan(&mut m, &cfg);
        assert_eq!(m[WAITING_FOR], "sammy");
        assert_eq!(backoff(&m), cfg.backoff_hours);
        let mut done: Meta = [("lifecycle".to_string(), "complete".to_string())].into();
        plan(&mut done, &cfg);
        assert!(!done.contains_key(BACKOFF));
    }
}
