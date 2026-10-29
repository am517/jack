fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", oliver::PROFILE),
        _ => {
            eprintln!("oliver: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
