//! Daniel calls Walter after adding or changing something: Walter runs on the call, then exits.
//! Optional: when Walter isn't installed, nothing happens.
use std::path::{Path, PathBuf};

/// `name` on PATH, else `windups-<name>`.
pub fn find(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path?).collect();
    [name.to_string(), format!("windups-{name}")]
        .iter()
        .find_map(|n| dirs.iter().map(|d| d.join(n)).find(|p| p.is_file()))
}

/// Runs the configured Walter command against `db`. Returns what happened, or None if skipped.
pub fn notify(cmd: &[String], db: &Path) -> Option<String> {
    let exe = find(cmd.first()?, std::env::var_os("PATH").as_deref())?;
    let r = std::process::Command::new(&exe)
        .args(&cmd[1..])
        .arg("--db")
        .arg(db)
        .output();
    Some(match r {
        Ok(o) if o.status.success() => "walter noted it".into(),
        Ok(o) => format!("walter: {}", String::from_utf8_lossy(&o.stderr).trim()),
        Err(e) => format!("walter: {e}"),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_by_name_or_windups_prefix_else_skips() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("windups-walter"), "").unwrap();
        let p = std::ffi::OsString::from(d.path());
        assert_eq!(
            super::find("walter", Some(&p)),
            Some(d.path().join("windups-walter"))
        );
        assert_eq!(super::find("nobody", Some(&p)), None);
        assert_eq!(super::notify(&["nobody-at-all".into()], d.path()), None);
    }
}
