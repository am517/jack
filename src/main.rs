fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--capabilities") => print!("{}", sammy::PROFILE),
        _ => {
            eprintln!("sammy: stub; only --capabilities for now");
            std::process::exit(2);
        }
    }
}
