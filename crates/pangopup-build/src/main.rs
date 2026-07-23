use pangopup_assets::{
    pack_bundle, prepare_release, unpack_transport, upload_release_asset, verify_transport,
};
use pangopup_build::{
    CommandError, build_bundle, inspect_directory, prepare_benchmark_corpus, prototype_open,
    prototype_roundtrip, verify_bundle,
};
use std::{env, path::Path, process::ExitCode};

const USAGE: &str = "Usage: pangopup-build inspect <SOURCE_DIR>\n       pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>\n       pangopup-build prototype-open <ARTIFACT>\n       pangopup-build benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>";

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|command| command == "build") {
        return build_command(&arguments[1..]);
    }
    if arguments.first().is_some_and(|command| command == "verify") {
        return verify_command(&arguments[1..]);
    }
    if arguments
        .first()
        .is_some_and(|command| command == "transport")
    {
        return transport_command(&arguments[1..]);
    }
    if arguments
        .first()
        .is_some_and(|command| command == "release")
    {
        return release_command(&arguments[1..]);
    }
    match arguments.as_slice() {
        [command, source] if command == "inspect" => {
            let mut stdout = std::io::stdout().lock();
            match inspect_directory(Path::new(source), &mut stdout) {
                Ok(_) => ExitCode::SUCCESS,
                Err(error) => legacy_failure("SOURCE_INVALID", &error),
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
                Err(error) => legacy_failure("SOURCE_INDEX", &error),
            }
        }
        [command, artifact] if command == "prototype-open" => {
            match prototype_open(Path::new(artifact)) {
                Ok(bytes) => {
                    println!("prototype-open format=fixed-11-v1 bytes={bytes} status=valid");
                    ExitCode::SUCCESS
                }
                Err(error) => legacy_failure("BUNDLE_INDEX", &error),
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
                Err(error) => legacy_failure("SOURCE_INVALID", &error),
            }
        }
        _ => json_failure(&CommandError::new("CLI_USAGE", USAGE)),
    }
}

fn release_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Some(action) = arguments.first().and_then(|value| value.to_str()) else {
        return json_usage("release requires prepare or upload-asset");
    };
    match action {
        "prepare" => {
            let Ok(values) =
                parse_exact_flags(&arguments[1..], &["--transport", "--receipt", "--output"])
            else {
                return json_usage(
                    "release prepare requires --transport, --receipt, and --output exactly once",
                );
            };
            match prepare_release(
                Path::new(values[0]),
                Path::new(values[1]),
                Path::new(values[2]),
            ) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => {
                    json_failure(&CommandError::new(error.kind().code(), error.to_string()))
                }
            }
        }
        "upload-asset" => {
            let Ok(values) = parse_exact_flags(
                &arguments[1..],
                &[
                    "--transport",
                    "--prepared",
                    "--gh",
                    "--release-id",
                    "--asset",
                ],
            ) else {
                return json_usage(
                    "release upload-asset requires --transport, --prepared, --gh, --release-id, and --asset exactly once",
                );
            };
            let Some(release_id) = values[3]
                .to_str()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
            else {
                return json_usage("release upload-asset requires a positive decimal --release-id");
            };
            let Some(asset) = values[4].to_str() else {
                return json_usage("release upload-asset requires a UTF-8 --asset name");
            };
            match upload_release_asset(
                Path::new(values[0]),
                Path::new(values[1]),
                Path::new(values[2]),
                release_id,
                asset,
            ) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => {
                    json_failure(&CommandError::new(error.kind().code(), error.to_string()))
                }
            }
        }
        _ => json_usage("release requires prepare or upload-asset"),
    }
}

fn transport_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Some(action) = arguments.first().and_then(|value| value.to_str()) else {
        return json_usage("transport requires pack, verify, or unpack");
    };
    let arguments = &arguments[1..];
    match action {
        "pack" => {
            let Ok(values) = parse_exact_flags(arguments, &["--bundle", "--output"]) else {
                return json_usage("transport pack requires --bundle and --output exactly once");
            };
            match pack_bundle(Path::new(values[0]), Path::new(values[1])) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => {
                    json_failure(&CommandError::new(error.kind().code(), error.to_string()))
                }
            }
        }
        "verify" => {
            let Ok(values) = parse_exact_flags(arguments, &["--transport"]) else {
                return json_usage("transport verify requires --transport exactly once");
            };
            match verify_transport(Path::new(values[0])) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => {
                    json_failure(&CommandError::new(error.kind().code(), error.to_string()))
                }
            }
        }
        "unpack" => {
            let Ok(values) = parse_exact_flags(arguments, &["--transport", "--output"]) else {
                return json_usage(
                    "transport unpack requires --transport and --output exactly once",
                );
            };
            match unpack_transport(Path::new(values[0]), Path::new(values[1])) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => {
                    json_failure(&CommandError::new(error.kind().code(), error.to_string()))
                }
            }
        }
        _ => json_usage("transport requires pack, verify, or unpack"),
    }
}

fn parse_exact_flags<'a>(
    arguments: &'a [std::ffi::OsString],
    flags: &[&str],
) -> Result<Vec<&'a std::ffi::OsStr>, ()> {
    let mut values = vec![None; flags.len()];
    let mut index = 0;
    while index < arguments.len() {
        let flag = arguments[index].to_str().ok_or(())?;
        let position = flags
            .iter()
            .position(|candidate| *candidate == flag)
            .ok_or(())?;
        index += 1;
        let value = arguments.get(index).ok_or(())?;
        if value.to_str().is_some_and(|value| value.starts_with("--")) {
            return Err(());
        }
        if values[position].replace(value.as_os_str()).is_some() {
            return Err(());
        }
        index += 1;
    }
    values.into_iter().collect::<Option<Vec<_>>>().ok_or(())
}

fn build_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let mut source = None;
    let mut reference = None;
    let mut output = None;
    let mut index = 0;
    while index < arguments.len() {
        let target = match arguments[index].to_str() {
            Some("--source") => &mut source,
            Some("--reference") => &mut reference,
            Some("--output") => &mut output,
            _ => {
                return json_usage(
                    "build requires --source, --reference, and --output exactly once",
                );
            }
        };
        index += 1;
        let Some(value) = arguments.get(index) else {
            return json_usage("build option is missing its path value");
        };
        if target.replace(value).is_some() {
            return json_usage("build option was supplied more than once");
        }
        index += 1;
    }
    let (Some(source), Some(reference), Some(output)) = (source, reference, output) else {
        return json_usage("build requires --source, --reference, and --output");
    };
    match build_bundle(Path::new(source), Path::new(reference), Path::new(output)) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&error),
    }
}

fn verify_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let [bundle] = arguments else {
        return json_usage("verify requires exactly one bundle path");
    };
    match verify_bundle(Path::new(bundle)) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&error),
    }
}

fn json_success(value: &impl serde::Serialize) -> ExitCode {
    match serde_json::to_writer(std::io::stdout().lock(), value) {
        Ok(()) => {
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => json_failure(&CommandError::new("IO", error.to_string())),
    }
}

fn json_usage(message: &str) -> ExitCode {
    json_failure(&CommandError::new("CLI_USAGE", message))
}

fn json_failure(error: &CommandError) -> ExitCode {
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, error);
    let _ = std::io::Write::write_all(&mut stderr, b"\n");
    if matches!(error.code, "CLI_USAGE" | "UNSUPPORTED_INPUT") {
        ExitCode::from(2)
    } else {
        ExitCode::from(1)
    }
}

fn legacy_failure(code: &'static str, error: &dyn std::fmt::Display) -> ExitCode {
    json_failure(&CommandError::new(code, format!("error: {error}")))
}
