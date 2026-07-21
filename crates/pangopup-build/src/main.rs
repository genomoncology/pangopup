use pangopup_build::inspect_directory;
use std::{env, path::Path, process::ExitCode};

const USAGE: &str = "Usage: pangopup-build inspect <SOURCE_DIR>";

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() != 2 || arguments[0] != "inspect" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut stdout = std::io::stdout().lock();
    match inspect_directory(Path::new(&arguments[1]), &mut stdout) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}
