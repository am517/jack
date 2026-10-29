//! Every windup's success checks are must-pass tests: each check in a PROFILE.toml names an
//! existing `#[test]` (`test = "crate/path.rs::fn_name"`, run by `cargo test --workspace`), or,
//! only for stubs, is explicitly `pending`.
use std::path::Path;

fn has_test(root: &Path, spec: &str) -> bool {
    let Some((file, name)) = spec.rsplit_once("::") else {
        return false;
    };
    let Ok(src) = std::fs::read_to_string(root.join(file)) else {
        return false;
    };
    let lines: Vec<&str> = src.lines().collect();
    lines.iter().enumerate().any(|(i, l)| {
        l.trim_start().starts_with(&format!("fn {name}("))
            && lines[i.saturating_sub(3)..i]
                .iter()
                .any(|p| p.trim() == "#[test]")
    })
}

#[test]
fn every_success_check_is_a_real_test_or_pending_for_stubs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut seen = 0;
    let mut bad = vec![];
    for dir in std::fs::read_dir(root).unwrap().flatten() {
        let Ok(text) = std::fs::read_to_string(dir.path().join("PROFILE.toml")) else {
            continue;
        };
        let p: toml::Value = toml::from_str(&text).unwrap();
        let name = p["name"].as_str().unwrap().to_string();
        let stub = p.get("status").and_then(|s| s.as_str()) == Some("stub");
        assert!(
            p.get("purpose")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty()),
            "{name}: no purpose"
        );
        let checks = p
            .get("success_checks")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if checks.is_empty() {
            bad.push(format!("{name}: no success_checks"));
        }
        for c in checks {
            let what = c.get("check").and_then(|v| v.as_str()).unwrap_or("?");
            match (c.get("test").and_then(|v| v.as_str()), c.get("pending")) {
                (Some(t), _) if has_test(root, t) => {}
                (Some(t), _) => bad.push(format!("{name}: '{what}' names a missing test {t}")),
                (None, Some(_)) if stub => {}
                (None, Some(_)) => bad.push(format!(
                    "{name}: '{what}' is pending but {name} is not a stub"
                )),
                (None, None) => bad.push(format!("{name}: '{what}' has no test")),
            }
        }
        seen += 1;
    }
    assert!(seen >= 14, "only {seen} profiles found");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
