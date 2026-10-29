//! The handoff: Jack only fetches and verifies; one entry per file goes to Daniel, the database
//! windup (his own crate), with the disagreements Jack found between sources.
use crate::meta::Meta;
use daniel::StoreCfg;
use std::path::Path;
use windups::handoff::{Daniel, Handoff};

static CALLER: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Who called this Jack run (`--caller`), carried in every handoff.
pub fn set_caller(c: Option<String>) {
    let _ = CALLER.set(c);
}

/// Hands one entry for `path` to Daniel.
pub fn hand_over(
    path: &Path,
    entry: &Meta,
    found: &[(String, Meta)],
    store: &StoreCfg,
) -> Result<String, String> {
    let conflicts = if entry.get("resolution").is_some_and(|r| r == "unresolved") {
        vec![]
    } else {
        crate::conflicts::find(entry, found)
    };
    daniel::Store::new(store).accept(&Handoff {
        file: path.to_path_buf(),
        entry: entry.clone(),
        candidates: found.to_vec(),
        conflicts,
        from: "jack".into(),
        caller: CALLER.get().cloned().flatten(),
    })
}
