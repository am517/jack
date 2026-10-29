fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", ben::PROFILE),
        _ => {
            eprintln!("ben: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
