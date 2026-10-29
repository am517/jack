//! Optional post-run review pass: after Jack finishes, a reviewer (Sammy) looks at one field at a
//! time across all items, in chunks, with minimal context (e.g. title + year beside a description).
//! The reviewer returns flags, never rewrites. A flag's suggestion is not applied: it is logged with
//! provenance and the item is left open, so any change still goes through Jack's sure-or-log rules.
use crate::explain::Item;
use crate::meta::Meta;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ReviewCfg {
    pub enabled: bool,
    /// Fields reviewed, one pass each.
    pub fields: Vec<String>,
    /// Items per reviewer call.
    pub chunk: usize,
    /// field -> extra fields shown beside it; others get `default_context`.
    pub context: BTreeMap<String, Vec<String>>,
    pub default_context: Vec<String>,
}

impl Default for ReviewCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            fields: vec![],
            chunk: 20,
            context: BTreeMap::new(),
            default_context: vec![],
        }
    }
}

/// One item as the reviewer sees it: the field's value plus minimal context.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub file: String,
    pub value: String,
    pub context: Meta,
}

/// A reviewer's concern about one value.
#[derive(Debug, Clone, PartialEq)]
pub struct Flag {
    pub file: String,
    pub field: String,
    pub issue: String,
    /// What the reviewer thinks it should be; logged only, never applied here.
    pub suggestion: Option<String>,
}

pub trait Reviewer {
    /// Provenance name ("sammy").
    fn source(&self) -> &str;
    /// False = skip the pass (no reviewer running).
    fn available(&self) -> bool;
    fn review(&self, field: &str, chunk: &[Entry]) -> Vec<Flag>;
}

/// No reviewer yet: never available.
pub struct NoReview;

impl Reviewer for NoReview {
    fn source(&self) -> &str {
        "none"
    }
    fn available(&self) -> bool {
        false
    }
    fn review(&self, _: &str, _: &[Entry]) -> Vec<Flag> {
        vec![]
    }
}

/// Runs the pass; returns open items (reason "review_flag") carrying provenance in their detail.
pub fn pass(
    cfg: &ReviewCfg,
    rv: &dyn Reviewer,
    done: &[(String, Meta)],
    out: &mut String,
) -> Vec<Item> {
    use std::fmt::Write;
    if !cfg.enabled || !rv.available() {
        return vec![];
    }
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut open = vec![];
    for field in &cfg.fields {
        let ctx = cfg.context.get(field).unwrap_or(&cfg.default_context);
        let entries: Vec<Entry> = done
            .iter()
            .filter_map(|(f, m)| {
                let value = m.get(field)?.clone();
                let context = ctx
                    .iter()
                    .filter_map(|k| m.get(k).map(|v| (k.clone(), v.clone())))
                    .collect();
                Some(Entry {
                    file: f.clone(),
                    value,
                    context,
                })
            })
            .collect();
        for chunk in entries.chunks(cfg.chunk.max(1)) {
            for fl in rv.review(field, chunk) {
                let sug = fl
                    .suggestion
                    .as_deref()
                    .map(|s| format!("; suggests {s:?} (not applied)"))
                    .unwrap_or_default();
                let detail = format!(
                    "{}: {}{sug} [source={}, step=review, at={at}]",
                    fl.field,
                    fl.issue,
                    rv.source()
                );
                let _ = writeln!(out, "review flag: {}: {detail}", fl.file);
                open.push(Item {
                    file: fl.file,
                    reason: "review_flag".into(),
                    detail,
                    tagged: true,
                });
            }
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake(RefCell<Vec<usize>>);
    impl Reviewer for Fake {
        fn source(&self) -> &str {
            "sammy"
        }
        fn available(&self) -> bool {
            true
        }
        fn review(&self, field: &str, c: &[Entry]) -> Vec<Flag> {
            self.0.borrow_mut().push(c.len());
            c.iter()
                .filter(|e| e.value.contains("lorem"))
                .map(|e| Flag {
                    file: e.file.clone(),
                    field: field.into(),
                    issue: "placeholder text".into(),
                    suggestion: Some("x".into()),
                })
                .collect()
        }
    }

    fn meta(p: &[(&str, &str)]) -> Meta {
        p.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn flags_only_in_chunks_with_minimal_context() {
        let cfg = ReviewCfg {
            enabled: true,
            fields: vec!["description".into()],
            chunk: 2,
            context: [("description".into(), vec!["title".into(), "year".into()])].into(),
            ..Default::default()
        };
        let done: Vec<(String, Meta)> = (0..3)
            .map(|i| {
                (
                    format!("f{i}"),
                    meta(&[
                        ("title", "T"),
                        ("year", "2001"),
                        ("genre", "g"),
                        ("description", if i == 1 { "lorem" } else { "ok" }),
                    ]),
                )
            })
            .collect();
        let rv = Fake(RefCell::new(vec![]));
        let mut out = String::new();
        let open = pass(&cfg, &rv, &done, &mut out);
        assert_eq!(*rv.0.borrow(), vec![2, 1]);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].file, "f1");
        assert!(open[0].detail.contains("not applied") && open[0].detail.contains("source=sammy"));
        assert_eq!(done[1].1["description"], "lorem", "never rewritten");
        // minimal context: no genre
        let e = Entry {
            file: "f".into(),
            value: "v".into(),
            context: meta(&[("title", "T")]),
        };
        assert!(!e.context.contains_key("genre"));
    }

    #[test]
    fn off_or_no_reviewer_does_nothing() {
        let done = vec![("f".to_string(), meta(&[("title", "lorem")]))];
        let on = ReviewCfg {
            enabled: true,
            fields: vec!["title".into()],
            ..Default::default()
        };
        assert!(pass(&on, &NoReview, &done, &mut String::new()).is_empty());
        let rv = Fake(RefCell::new(vec![]));
        assert!(
            pass(
                &ReviewCfg {
                    enabled: false,
                    ..on
                },
                &rv,
                &done,
                &mut String::new()
            )
            .is_empty()
        );
        assert!(rv.0.borrow().is_empty());
    }

    #[test]
    fn shipped_config_is_off() {
        let t: toml::Value = toml::from_str(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/config/jack.toml"))
                .unwrap(),
        )
        .unwrap();
        let c: ReviewCfg = t["review"].clone().try_into().unwrap();
        assert!(!c.enabled);
        assert!(c.fields.contains(&"description".to_string()) && c.chunk > 0);
    }
}
