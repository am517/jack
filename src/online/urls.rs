//! Recognizing a source URL: (site, id).
use regex::Regex;
use std::sync::OnceLock;

pub fn parse(s: &str) -> Option<(String, String)> {
    static R: OnceLock<[(&str, Regex); 2]> = OnceLock::new();
    let r = R.get_or_init(|| {
        [
            ("youtube", Regex::new(r"(?:youtube\.com/(?:watch\?(?:.*&)?v=|shorts/|live/|embed/)|youtu\.be/)([A-Za-z0-9_-]{11})").unwrap()),
            ("x", Regex::new(r"(?:twitter|x)\.com/[^/\s]+/status/(\d+)").unwrap()),
        ]
    });
    r.iter().find_map(|(site, re)| re.captures(s).map(|c| (site.to_string(), c[1].to_string())))
}

#[cfg(test)]
mod tests {
    #[test]
    fn urls() {
        let p = |s| super::parse(s).map(|(a, b)| format!("{a}:{b}"));
        assert_eq!(p("see https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1").as_deref(), Some("youtube:dQw4w9WgXcQ"));
        assert_eq!(p("https://youtu.be/dQw4w9WgXcQ").as_deref(), Some("youtube:dQw4w9WgXcQ"));
        assert_eq!(p("https://twitter.com/a/status/42").as_deref(), Some("x:42"));
        assert_eq!(p("https://example.com"), None);
    }
}
