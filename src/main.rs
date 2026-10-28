mod art;
mod askart;
mod cli;
mod config;
mod fetch;
mod matcher;
mod meta;
mod print;
mod process;
mod providers;
mod run;
mod scan;
mod secrets;
mod setcfg;
mod setflags;
mod sorter;
mod sources;
mod store;
mod tagger;
#[cfg(test)]
mod testutil;

use clap::Parser;

fn main() {
    let mut cli = cli::Cli::parse();
    if let Err(e) = secrets::resolve_secrets(&mut cli, &mut secrets::read_secret) {
        eprintln!("jack: {e}");
        std::process::exit(1);
    }
    match run::run(&cli, &providers::Ureq) {
        Ok((s, printed, failed)) => {
            // with --print, stdout carries only the metadata; the report goes to stderr
            if printed.is_empty() {
                print!("{s}");
            } else {
                eprint!("{s}");
                print!("{printed}");
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("jack: {e}");
            std::process::exit(1);
        }
    }
}
