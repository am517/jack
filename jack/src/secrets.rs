//! Credential flags: prompting and collecting them.
use crate::cli::*;

/// (flag, config key) for each credential flag.
pub(crate) const SECRETS: [(&str, &str); 4] = [
    ("--tmdb-token", "providers.tmdb.read_token"),
    ("--tvdb-key", "providers.thetvdb.api_key"),
    ("--tvdb-pin", "providers.thetvdb.pin"),
    ("--omdb-key", "providers.omdb.api_key"),
];

pub(crate) fn secret_fields(cli: &mut Cli) -> [&mut Option<Option<String>>; 4] {
    [
        &mut cli.tmdb_token,
        &mut cli.tvdb_key,
        &mut cli.tvdb_pin,
        &mut cli.omdb_key,
    ]
}

/// Fills value-less credential flags via `read` (hidden prompt in `main`).
pub(crate) fn resolve_secrets(
    cli: &mut Cli,
    read: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    for (f, (flag, _)) in secret_fields(cli).into_iter().zip(SECRETS) {
        if let Some(None) = f {
            let v = read(&format!("{flag}: "))?.trim().to_string();
            if v.is_empty() {
                return Err(format!("{flag}: empty value"));
            }
            *f = Some(Some(v));
        }
    }
    Ok(())
}

/// Hidden prompt on a terminal; otherwise one line from stdin (e.g. `echo $KEY | jack --set --omdb-key`).
pub(crate) fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password(prompt).map_err(|e| format!("reading key: {e}"))
    } else {
        let mut s = String::new();
        std::io::stdin()
            .read_line(&mut s)
            .map_err(|e| format!("reading key from stdin: {e}"))?;
        Ok(s)
    }
}

pub(crate) fn secret_values(cli: &Cli) -> Vec<(&'static str, &'static str, String)> {
    [&cli.tmdb_token, &cli.tvdb_key, &cli.tvdb_pin, &cli.omdb_key]
        .into_iter()
        .zip(SECRETS)
        .filter_map(|(v, (f, k))| v.clone().flatten().map(|v| (f, k, v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn credential_flags_run_only_set_masked_and_prompted() {
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let c = d.path().join("c.toml");
        fs::write(&c, "# my keys\n[providers]\norder = [\"omdb\"]\n[providers.omdb]\napi_key = \"CFGKEY1234\" # keep me\nbase_url = \"http://o/\"\n").unwrap();
        let m = d.path().join("Dune (1984).mp4");
        fs::write(&m, "").unwrap();
        let (cs, ms) = (c.to_str().unwrap(), m.to_str().unwrap());
        let h = providers::fake::FakeHttp::new(vec![(
            "http://o/",
            json!({"Title":"Dune","Year":"1984"}),
        )]);
        // run-only: flag beats config; config untouched; key never printed
        let (o, _, _) = run(
            &cli(&["--config", cs, "--dry-run", "--omdb-key=FLAGKEY5678", ms]),
            &h,
        )
        .unwrap();
        assert!(
            h.calls
                .borrow()
                .iter()
                .any(|u| u.contains("apikey=FLAGKEY5678"))
        );
        assert!(
            !o.contains("FLAGKEY5678") && !o.contains("CFGKEY1234"),
            "{o}"
        );
        assert!(fs::read_to_string(&c).unwrap().contains("CFGKEY1234"));
        // value-less flag prompts through the reader; --set saves masked
        let mut a = cli(&["--set", "--config", cs, "--omdb-key", "--tvdb-pin=9999"]);
        let mut asked = vec![];
        resolve_secrets(&mut a, &mut |p: &str| {
            asked.push(p.to_string());
            Ok("NEWKEY0001\n".into())
        })
        .unwrap();
        assert_eq!(asked, ["--omdb-key: "]);
        let (o, _, _) = run(&a, &h).unwrap();
        assert!(
            o.contains("providers.omdb.api_key: ******** -> ********")
                && o.contains("providers.thetvdb.pin: (unset) -> ********"),
            "{o}"
        );
        assert!(
            !o.contains("NEWKEY0001") && !o.contains("CFGKEY1234") && !o.contains("9999"),
            "{o}"
        );
        let t = fs::read_to_string(&c).unwrap();
        assert!(
            t.contains("api_key = \"NEWKEY0001\" # keep me")
                && t.starts_with("# my keys")
                && t.contains("pin = \"9999\""),
            "{t}"
        );
        let mut e = cli(&["--omdb-key"]);
        assert!(resolve_secrets(&mut e, &mut |_: &str| Ok(" \n".into())).is_err());
    }
}
