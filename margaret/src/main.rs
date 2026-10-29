fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", margaret::PROFILE),
        _ => {
            eprintln!("margaret: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
