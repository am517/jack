mod art;
mod askart;
mod candidates;
mod cli;
mod completeness;
mod config;
mod conflicts;
mod decide;
mod escalate;
mod explain;
mod facts;
mod fetch;
mod filemeta;
mod fingerprint;
mod hash;
mod matcher;
mod meta;
mod online;
mod pending;
mod print;
mod process;
mod profile;
mod providers;
mod resolver;
mod returns;
mod review;
mod run;
mod scan;
mod secrets;
mod setcfg;
mod setflags;
mod settle;
mod sorter;
mod sources;
mod stage;
mod store;
mod tagger;
#[cfg(test)]
mod testutil;
mod workkey;

use clap::Parser;

fn main() {
    // `jack pending` / `jack retry [--all]`: unfinished work comes back
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(cmd @ ("pending" | "retry")) = args.first().map(String::as_str) {
        match pending::run(cmd, &args[1..], &providers::Ureq) {
            Ok((s, failed)) => {
                print!("{s}");
                std::process::exit(i32::from(failed > 0));
            }
            Err(e) => {
                eprintln!("jack: {e}");
                std::process::exit(1);
            }
        }
    }
    let mut cli = cli::Cli::parse();
    if cli.capabilities {
        print!("{}", profile::PROFILE);
        return;
    }
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
