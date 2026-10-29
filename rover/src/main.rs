fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", rover::PROFILE),
        _ => {
            eprintln!("rover: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
