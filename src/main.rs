fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("--capabilities") => Ok(walter::PROFILE.to_string()),
        Some(cmd @ ("pending" | "patrol" | "note")) => walter::run(cmd, &args[1..], &walter::caller::Process),
        _ => Err("usage: walter [note | pending | patrol [--all] [--wake CAPABILITY]] [--db PATH] [--config PATH] | --capabilities".into()),
    };
    match r {
        Ok(s) => print!("{s}"),
        Err(e) => {
            eprintln!("walter: {e}");
            std::process::exit(1);
        }
    }
}
