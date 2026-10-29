//! Walter end to end on a real database with a fake Jack.
use std::cell::RefCell;
use std::path::PathBuf;
use walter::caller::Caller;

#[derive(Default)]
struct FakeJack {
    sources: RefCell<Vec<&'static str>>,
    called: RefCell<Vec<PathBuf>>,
}

impl Caller for FakeJack {
    fn capabilities(&self, _: &[String]) -> Option<String> {
        let s: Vec<String> = self
            .sources
            .borrow()
            .iter()
            .map(|s| format!("{s:?}"))
            .collect();
        Some(format!(
            "name = \"jack\"\nversion = \"0.1\"\nsources = [{}]\n",
            s.join(", ")
        ))
    }
    fn call(&self, _: &[String], files: &[PathBuf]) -> Result<String, String> {
        self.called.borrow_mut().extend(files.iter().cloned());
        Ok("jack ran\n".into())
    }
}

#[test]
fn nothing_incomplete_is_forgotten() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("m.db");
    let cfg = d.path().join("w.toml");
    std::fs::write(&cfg, "[retry]\nbackoff_hours = [0, 24]\n").unwrap();
    let f = d.path().join("Nothing.Like.It.mkv");
    std::fs::write(&f, "x").unwrap();
    {
        let c = windups::db::open(&db).unwrap();
        for (t, stage) in [
            ("a", "unverified"),
            ("b", "complete"),
            ("c", "not_identified"),
        ] {
            c.execute(
                "INSERT INTO media(kind, title, source) VALUES ('movie', ?1, 'x')",
                [t],
            )
            .unwrap();
            let id = c.last_insert_rowid();
            windups::lifecycle::set(&c, id, stage, "jack", "test").unwrap();
        }
        c.execute(
            "INSERT INTO files(path, media_id) VALUES (?1, 1)",
            [f.to_str().unwrap()],
        )
        .unwrap();
    }
    let args = |extra: &[&str]| {
        let mut v = vec![
            "--db".to_string(),
            db.to_str().unwrap().into(),
            "--config".into(),
            cfg.to_str().unwrap().into(),
        ];
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    };
    let jack = FakeJack::default();
    jack.sources.borrow_mut().push("tmdb");
    let out = walter::run("pending", &args(&[]), &jack).unwrap();
    assert!(out.contains("2 pending"), "complete has no return: {out}");
    assert!(
        out.contains("[unverified]") && out.contains("when new_source appears"),
        "{out}"
    );
    assert!(
        out.contains("[not_identified]") && out.contains("when sammy appears"),
        "{out}"
    );
    // first back-off is 0 h: both are due (one has a file) and Jack is called with its file
    let out = walter::run("patrol", &args(&[]), &jack).unwrap();
    assert!(
        out.contains("2 due, 1 with a file present") && out.contains("jack ran"),
        "{out}"
    );
    assert_eq!(*jack.called.borrow(), [f.clone()]);
    // then it backs off a day
    let out = walter::run("patrol", &args(&[]), &jack).unwrap();
    assert!(out.contains("0 due"), "{out}");
    // a new source appears in Jack's profile: what waits for new_source wakes
    jack.sources.borrow_mut().push("omdb");
    let out = walter::run("patrol", &args(&[]), &jack).unwrap();
    assert!(
        out.contains("woke items waiting for: omdb") && out.contains("1 due"),
        "{out}"
    );
    // Sammy named by hand
    let out = walter::run("pending", &args(&["--wake", "sammy"]), &jack).unwrap();
    assert!(out.contains("woke items waiting for: sammy"), "{out}");
}
