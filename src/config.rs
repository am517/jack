//! Daniel's tweakables: config/daniel.toml, overlaid by the user's file.
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub walter: WalterCfg,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct WalterCfg {
    pub enabled: bool,
    pub command: Vec<String>,
}

const DEFAULTS: &str = include_str!("../config/daniel.toml");

impl Config {
    pub fn load(user: Option<&Path>) -> Result<Self, String> {
        let mut v: toml::Table = toml::from_str(DEFAULTS).map_err(|e| e.to_string())?;
        if let Some(t) = user.and_then(|p| std::fs::read_to_string(p).ok()) {
            let u: toml::Table = toml::from_str(&t).map_err(|e| e.to_string())?;
            for (k, x) in u {
                match (v.get_mut(&k), x) {
                    (Some(toml::Value::Table(a)), toml::Value::Table(b)) => a.extend(b),
                    (_, x) => {
                        v.insert(k, x);
                    }
                }
            }
        }
        toml::Value::Table(v).try_into().map_err(|e| e.to_string())
    }
}
