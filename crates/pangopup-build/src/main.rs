use pangopup_build::{
    inspect_directory, prepare_benchmark_corpus, prototype_open, prototype_roundtrip,
};
use std::{env, path::Path, process::ExitCode};

const USAGE: &str = "Usage: pangopup-build inspect <SOURCE_DIR>\n       pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>\n       pangopup-build prototype-open <ARTIFACT>\n       pangopup-build benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>";

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [command, source] if command == "inspect" => {
            let mut stdout = std::io::stdout().lock();
            match inspect_directory(Path::new(source), &mut stdout) {
                Ok(_) => ExitCode::SUCCESS,
                Err(error) => fail(&error),
            }
        }
        [command, source, output] if command == "prototype-roundtrip" => {
            match prototype_roundtrip(Path::new(source), Path::new(output)) {
                Ok(summary) => {
                    println!(
                        "prototype format=fixed-11-v1 bytes={} genes={} rows={} loci={} segments={} exceptions={} verified_rows={}",
                        summary.artifact.bytes,
                        summary.source.genes,
                        summary.source.rows,
                        summary.source.loci,
                        summary.artifact.segments,
                        summary.artifact.exceptions,
                        summary.source.rows
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        [command, artifact] if command == "prototype-open" => {
            match prototype_open(Path::new(artifact)) {
                Ok(bytes) => {
                    println!("prototype-open format=fixed-11-v1 bytes={bytes} status=valid");
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        [command, source, output, manifest] if command == "benchmark-corpus" => {
            match prepare_benchmark_corpus(
                Path::new(source),
                Path::new(output),
                Path::new(manifest),
            ) {
                Ok(summary) => {
                    println!(
                        "benchmark-corpus genes={} loci={} rows={} observed_member_sha256={}",
                        summary.selected_genes,
                        summary.loci,
                        summary.rows,
                        summary.observed_member_sha256
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => fail(&error),
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn fail(error: &dyn std::fmt::Display) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::from(1)
}
