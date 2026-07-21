use std::{env, process::ExitCode};

const HELP: &str = "Pangopup: exact Pangolin score lookup\n\nUsage: pangopup [--help | --version]\n\nScore lookup is not implemented yet.";

fn main() -> ExitCode {
    match env::args().nth(1).as_deref() {
        None | Some("-h" | "--help") => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("pangopup {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(argument) => {
            eprintln!("unknown argument: {argument}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}
