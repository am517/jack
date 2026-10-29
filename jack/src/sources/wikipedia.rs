//! Wikipedia: the article summary, used only when no provider gave a description.
use crate::providers::{Http, enc, s};
use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct WikipediaCfg {
    pub enabled: bool,
    /// REST base, e.g. https://en.wikipedia.org/api/rest_v1
    pub base_url: String,
}

/// Summary of the article Wikidata pointed at.
pub fn description(
    cfg: &WikipediaCfg,
    http: &dyn Http,
    title: &str,
) -> Result<Option<String>, String> {
    if !cfg.enabled {
        return Ok(None);
    }
    let r = http.get(
        &format!(
            "{}/page/summary/{}",
            cfg.base_url,
            enc(&title.replace(' ', "_"))
        ),
        &[],
    )?;
    Ok(s(&r["extract"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::fake::FakeHttp;
    #[test]
    fn summary_extract() {
        let h = FakeHttp::new(vec![(
            "/page/summary/The_Matrix",
            serde_json::json!({"extract":"A 1999 film."}),
        )]);
        let c = WikipediaCfg {
            enabled: true,
            base_url: "http://wp".into(),
        };
        assert_eq!(
            description(&c, &h, "The Matrix").unwrap().as_deref(),
            Some("A 1999 film.")
        );
        assert!(
            description(&WikipediaCfg::default(), &h, "x")
                .unwrap()
                .is_none()
        );
    }
}
