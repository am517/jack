//! The handoff to Daniel, the database windup: a windup that has fetched and verified what it can
//! about one file hands over one entry; Daniel alone writes it to the shared database (and asks
//! Larry, Tom, Mary or Kate for what they add). Windups call windups; nothing else schedules work.
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type Fields = BTreeMap<String, String>;
/// (field, [(source, value)]): sources about the same work that disagree; recorded, not settled.
pub type Conflict = (String, Vec<(String, String)>);

pub struct Handoff {
    /// The file the entry is about.
    pub file: PathBuf,
    /// The chosen, verified entry (including `lifecycle` and the reasons for it).
    pub entry: Fields,
    /// Every source's answer, kept whole (source name, its fields).
    pub candidates: Vec<(String, Fields)>,
    /// Disagreements the handing windup found between sources (it verifies; Daniel only records).
    pub conflicts: Vec<Conflict>,
    /// Who hands it over (e.g. "jack").
    pub from: String,
}

pub trait Daniel {
    /// Stores the entry; returns a one-line receipt (e.g. "database: media #12 in ...").
    fn accept(&self, h: &Handoff) -> Result<String, String>;
}
