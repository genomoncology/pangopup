use pangopup_assets::{
    pack_bundle, prepare_release, unpack_transport, upload_release_asset, verify_transport,
};
use pangopup_build::{
    CommandError, build_bundle,
    compatibility::{CaptureArguments, capture_corpus, inspect_corpus},
    inspect_directory, prepare_benchmark_corpus, prototype_open, prototype_roundtrip,
    reference::{build_reference_bundle, inspect_reference_bundle, reference_window},
    verify_bundle,
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
        .is_some_and(|command| command == "reference")
    {
        return reference_command(&arguments[1..]);
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
    if arguments
        .first()
        .is_some_and(|command| command == "compatibility")
    {
        return compatibility_command(&arguments[1..]);
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

fn reference_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Some(action) = arguments.first().and_then(|value| value.to_str()) else {
        return reference_failure(
            "reference",
            "CLI_USAGE",
            "reference requires build, inspect, or window",
            2,
        );
    };
    match action {
        "build" => {
            let Ok(values) = parse_exact_flags(
                &arguments[1..],
                &["--profile", "--source", "--assembly-report", "--output"],
            ) else {
                return reference_failure(
                    "reference.build",
                    "CLI_USAGE",
                    "reference build arguments are invalid",
                    2,
                );
            };
            let Some(profile) = values[0].to_str() else {
                return reference_failure(
                    "reference.build",
                    "CLI_USAGE",
                    "reference profile is invalid",
                    2,
                );
            };
            match build_reference_bundle(
                profile,
                Path::new(values[1]),
                Path::new(values[2]),
                Path::new(values[3]),
            ) {
                Ok(value) => reference_json(&value, 0),
                Err(error) => reference_command_error("reference.build", &error),
            }
        }
        "inspect" => {
            let Ok(values) = parse_exact_flags(&arguments[1..], &["--bundle"]) else {
                return reference_failure(
                    "reference.inspect",
                    "CLI_USAGE",
                    "reference inspect arguments are invalid",
                    2,
                );
            };
            match inspect_reference_bundle(Path::new(values[0])) {
                Ok(value) => reference_json(&value, 0),
                Err(error) => reference_command_error("reference.inspect", &error),
            }
        }
        "window" => {
            let Ok(values) = parse_exact_flags(
                &arguments[1..],
                &["--bundle", "--contig", "--start", "--length"],
            ) else {
                return reference_failure(
                    "reference.window",
                    "CLI_USAGE",
                    "reference window arguments are invalid",
                    2,
                );
            };
            let Some(alias) = values[1]
                .to_str()
                .filter(|value| valid_reference_alias(value))
            else {
                return reference_failure(
                    "reference.window",
                    "CLI_USAGE",
                    "reference contig is invalid",
                    2,
                );
            };
            let Some(start) = values[2]
                .to_str()
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|value| *value > 0)
            else {
                return reference_failure(
                    "reference.window",
                    "CLI_USAGE",
                    "reference start is invalid",
                    2,
                );
            };
            let Some(length) = values[3]
                .to_str()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| (1..=1_048_576).contains(value))
            else {
                return reference_failure(
                    "reference.window",
                    "CLI_USAGE",
                    "reference length is invalid",
                    2,
                );
            };
            match reference_window(Path::new(values[0]), alias, start, length) {
                Ok(value) => reference_json(&value, 0),
                Err(error) => reference_command_error("reference.window", &error),
            }
        }
        _ => reference_failure("reference", "CLI_USAGE", "reference action is invalid", 2),
    }
}

fn valid_reference_alias(value: &str) -> bool {
    if value.parse::<pangopup_core::Grch38Contig>().is_ok() {
        return true;
    }
    (1_u8..=25).any(|code| {
        pangopup_core::Grch38Contig::from_code(code)
            .ok()
            .is_some_and(|contig| pangopup_index::reference::required_accession(contig) == value)
    })
}

fn reference_command_error(command: &'static str, error: &CommandError) -> ExitCode {
    let code = match error.code {
        "CLI_USAGE" => "CLI_USAGE",
        "REFERENCE_INPUT" => "REFERENCE_INPUT",
        "REFERENCE_BUNDLE" => "REFERENCE_BUNDLE",
        "REFERENCE_WINDOW" => "REFERENCE_WINDOW",
        "ALREADY_EXISTS" => "ALREADY_EXISTS",
        _ => "IO",
    };
    reference_failure(
        command,
        code,
        reference_error_message(code),
        u8::from(code == "CLI_USAGE") + 1,
    )
}

fn reference_error_message(code: &str) -> &'static str {
    match code {
        "CLI_USAGE" => "reference command arguments are invalid",
        "REFERENCE_INPUT" => "reference input is invalid",
        "REFERENCE_BUNDLE" => "reference bundle is invalid",
        "REFERENCE_WINDOW" => "reference window is invalid",
        "ALREADY_EXISTS" => "reference output already exists",
        _ => "reference I/O failed",
    }
}

fn reference_failure(
    command: &'static str,
    code: &'static str,
    message: &'static str,
    exit: u8,
) -> ExitCode {
    #[derive(serde::Serialize)]
    struct ErrorBody {
        code: &'static str,
        message: &'static str,
    }
    #[derive(serde::Serialize)]
    struct Failure {
        ok: bool,
        command: &'static str,
        error: ErrorBody,
    }
    reference_json(
        &Failure {
            ok: false,
            command,
            error: ErrorBody { code, message },
        },
        exit,
    )
}

fn reference_json(value: &impl serde::Serialize, exit: u8) -> ExitCode {
    let bytes = match serde_jcs::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => b"{\"command\":\"reference\",\"error\":{\"code\":\"IO\",\"message\":\"JSON output failed\"},\"ok\":false}".to_vec(),
    };
    let mut stdout = std::io::stdout().lock();
    if std::io::Write::write_all(&mut stdout, &bytes)
        .and_then(|_| std::io::Write::write_all(&mut stdout, b"\n"))
        .is_err()
    {
        return ExitCode::from(1);
    }
    ExitCode::from(exit)
}

fn compatibility_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Some(action) = arguments.first().and_then(|value| value.to_str()) else {
        return json_usage("compatibility requires inspect or capture");
    };
    match action {
        "inspect" => {
            let Ok(values) = parse_exact_flags(&arguments[1..], &["--corpus"]) else {
                return json_usage("compatibility inspect requires --corpus exactly once");
            };
            match inspect_corpus(Path::new(values[0])) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => json_failure(&error),
            }
        }
        "capture" => {
            let flags = [
                "--upstream",
                "--python",
                "--reference-source",
                "--assembly-report",
                "--reference",
                "--annotation-db",
                "--annotation-gtf",
                "--output",
            ];
            let Ok(values) = parse_exact_flags(&arguments[1..], &flags) else {
                return json_usage(
                    "compatibility capture requires --upstream, --python, --reference-source, --assembly-report, --reference, --annotation-db, --annotation-gtf, and --output exactly once",
                );
            };
            let capture = CaptureArguments {
                upstream: Path::new(values[0]).to_owned(),
                python: Path::new(values[1]).to_owned(),
                reference_source: Path::new(values[2]).to_owned(),
                assembly_report: Path::new(values[3]).to_owned(),
                reference: Path::new(values[4]).to_owned(),
                annotation_db: Path::new(values[5]).to_owned(),
                annotation_gtf: Path::new(values[6]).to_owned(),
                output: Path::new(values[7]).to_owned(),
            };
            match capture_corpus(&capture) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => json_failure(&error),
            }
        }
        _ => json_usage("compatibility requires inspect or capture"),
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
