use pangopup_assets::{
    pack_bundle, pack_runtime_transport, prepare_release, prepare_runtime_release,
    unpack_runtime_transport, unpack_transport, verify_runtime_transport, verify_transport,
};
use pangopup_build::{
    CommandError, build_bundle,
    compatibility::{CaptureArguments, capture_corpus, inspect_corpus},
    executable_release::prepare_executable_release,
    inspect_directory,
    model::{
        ConvertArguments, EvidenceArguments, convert_model_bundle, create_model_evidence,
        inspect_model_bundle, qualify_model_bundle,
    },
    prepare_benchmark_corpus, prototype_open, prototype_roundtrip,
    reference::{build_reference_bundle, inspect_reference_bundle, reference_window},
    runtime_profile::prepare_runtime_profile,
    verify_bundle,
};
use pangopup_model::ModelRepresentation;
use std::{env, path::Path, process::ExitCode};

mod cli;

use cli::Leaf;

const LEGACY_USAGE: &str = "Usage: pangopup-build inspect <SOURCE_DIR>\n       pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>\n       pangopup-build prototype-open <ARTIFACT>\n       pangopup-build benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>";

fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if let Some(information) = cli::information(&arguments) {
        println!("{}", cli::render(information));
        return ExitCode::SUCCESS;
    }
    if let Some((leaf, operands)) = cli::resolve(&arguments) {
        return dispatch(leaf, operands);
    }
    match cli::namespace(&arguments) {
        Some("reference") => reference_invalid(&arguments[1..]),
        Some("transport") => json_usage("transport requires pack, verify, or unpack"),
        Some("release") => json_usage("release requires prepare"),
        Some("compatibility") => json_usage("compatibility requires inspect or capture"),
        Some("model") => json_usage("model requires evidence, convert, inspect, or qualify"),
        Some("runtime-profile") => json_usage("runtime-profile requires prepare"),
        Some("runtime-transport") => {
            json_usage("runtime-transport requires pack, verify, or unpack")
        }
        Some("runtime-release") => json_usage("runtime-release requires prepare"),
        Some("executable-release") => json_usage("executable-release requires prepare"),
        Some(_) => unreachable!("closed namespace catalog"),
        None => json_failure(&CommandError::new("CLI_USAGE", LEGACY_USAGE)),
    }
}

fn dispatch(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::Inspect => match arguments {
            [source] => {
                let mut stdout = std::io::stdout().lock();
                match inspect_directory(Path::new(source), &mut stdout) {
                    Ok(_) => ExitCode::SUCCESS,
                    Err(error) => legacy_failure("SOURCE_INVALID", &error),
                }
            }
            _ => json_failure(&CommandError::new("CLI_USAGE", LEGACY_USAGE)),
        },
        Leaf::PrototypeRoundtrip => match arguments {
            [source, output] => match prototype_roundtrip(Path::new(source), Path::new(output)) {
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
            },
            _ => json_failure(&CommandError::new("CLI_USAGE", LEGACY_USAGE)),
        },
        Leaf::PrototypeOpen => match arguments {
            [artifact] => match prototype_open(Path::new(artifact)) {
                Ok(bytes) => {
                    println!("prototype-open format=fixed-11-v1 bytes={bytes} status=valid");
                    ExitCode::SUCCESS
                }
                Err(error) => legacy_failure("BUNDLE_INDEX", &error),
            },
            _ => json_failure(&CommandError::new("CLI_USAGE", LEGACY_USAGE)),
        },
        Leaf::BenchmarkCorpus => match arguments {
            [source, output, manifest] => {
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
            _ => json_failure(&CommandError::new("CLI_USAGE", LEGACY_USAGE)),
        },
        Leaf::Build => build_command(arguments),
        Leaf::Verify => verify_command(arguments),
        Leaf::ReferenceBuild | Leaf::ReferenceInspect | Leaf::ReferenceWindow => {
            reference_command(leaf, arguments)
        }
        Leaf::TransportPack | Leaf::TransportVerify | Leaf::TransportUnpack => {
            transport_command(leaf, arguments)
        }
        Leaf::ReleasePrepare => release_command(leaf, arguments),
        Leaf::CompatibilityInspect | Leaf::CompatibilityCapture => {
            compatibility_command(leaf, arguments)
        }
        Leaf::ModelEvidence | Leaf::ModelConvert | Leaf::ModelInspect | Leaf::ModelQualify => {
            model_command(leaf, arguments)
        }
        Leaf::RuntimeProfilePrepare => runtime_profile_command(arguments),
        Leaf::RuntimeTransportPack
        | Leaf::RuntimeTransportVerify
        | Leaf::RuntimeTransportUnpack => runtime_transport_command(leaf, arguments),
        Leaf::RuntimeReleasePrepare => runtime_release_command(arguments),
        Leaf::ExecutableReleasePrepare => executable_release_command(arguments),
    }
}

fn executable_release_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Ok(values) = parse_exact_flags(
        arguments,
        &[
            "--executable",
            "--sbom",
            "--version",
            "--target-commit",
            "--repository",
            "--output",
        ],
    ) else {
        return json_usage(
            "executable-release prepare requires --executable, --sbom, --version, --target-commit, --repository, and --output exactly once",
        );
    };
    match prepare_executable_release(
        Path::new(values[0]),
        Path::new(values[1]),
        values[2].to_str().unwrap_or_default(),
        values[3].to_str().unwrap_or_default(),
        Path::new(values[4]),
        Path::new(values[5]),
    ) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&error),
    }
}

fn runtime_release_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Ok(values) = parse_exact_flags(arguments, &["--transport", "--target-commit", "--output"])
    else {
        return json_usage(
            "runtime-release prepare requires --transport, --target-commit, and --output exactly once",
        );
    };
    match prepare_runtime_release(
        Path::new(values[0]),
        values[1].to_str().unwrap_or_default(),
        Path::new(values[2]),
    ) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&CommandError::new(error.kind().code(), error.to_string())),
    }
}

fn runtime_transport_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    let result = match leaf {
        Leaf::RuntimeTransportPack => {
            let Ok(values) = parse_exact_flags(
                arguments,
                &[
                    "--profile",
                    "--model-bundle",
                    "--reference-bundle",
                    "--mask",
                    "--output",
                ],
            ) else {
                return json_usage(
                    "runtime-transport pack requires --profile, --model-bundle, --reference-bundle, --mask, and --output exactly once",
                );
            };
            pack_runtime_transport(
                Path::new(values[0]),
                Path::new(values[1]),
                Path::new(values[2]),
                Path::new(values[3]),
                Path::new(values[4]),
            )
            .map(|outcome| serde_json::to_value(outcome).expect("serializable outcome"))
        }
        Leaf::RuntimeTransportVerify => {
            let Ok(values) = parse_exact_flags(arguments, &["--transport"]) else {
                return json_usage("runtime-transport verify requires --transport exactly once");
            };
            verify_runtime_transport(Path::new(values[0]))
                .map(|outcome| serde_json::to_value(outcome).expect("serializable outcome"))
        }
        Leaf::RuntimeTransportUnpack => {
            let Ok(values) = parse_exact_flags(arguments, &["--transport", "--output"]) else {
                return json_usage(
                    "runtime-transport unpack requires --transport and --output exactly once",
                );
            };
            unpack_runtime_transport(Path::new(values[0]), Path::new(values[1]))
                .map(|outcome| serde_json::to_value(outcome).expect("serializable outcome"))
        }
        _ => unreachable!("runtime transport dispatcher receives runtime transport leaves"),
    };
    match result {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&CommandError::new(error.kind().code(), error.to_string())),
    }
}

fn runtime_profile_command(arguments: &[std::ffi::OsString]) -> ExitCode {
    let Ok(values) = parse_exact_flags(
        arguments,
        &[
            "--snv-bundle",
            "--model-bundle",
            "--reference-bundle",
            "--mask",
            "--output",
        ],
    ) else {
        return json_usage(
            "runtime-profile prepare requires --snv-bundle, --model-bundle, --reference-bundle, --mask, and --output exactly once",
        );
    };
    match prepare_runtime_profile(
        Path::new(values[0]),
        Path::new(values[1]),
        Path::new(values[2]),
        Path::new(values[3]),
        Path::new(values[4]),
    ) {
        Ok(outcome) => json_success(&outcome),
        Err(error) => json_failure(&error),
    }
}

fn model_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::ModelEvidence => {
            let Ok(values) = parse_exact_flags(
                arguments,
                &["--upstream", "--python", "--corpus", "--output"],
            ) else {
                return json_usage(
                    "model evidence requires --upstream, --python, --corpus, and --output exactly once",
                );
            };
            let arguments = EvidenceArguments {
                upstream: Path::new(values[0]).to_owned(),
                python: Path::new(values[1]).to_owned(),
                corpus: Path::new(values[2]).to_owned(),
                output: Path::new(values[3]).to_owned(),
            };
            match create_model_evidence(&arguments) {
                Ok(value) => model_json(&value, 0),
                Err(error) => model_failure(&error),
            }
        }
        Leaf::ModelConvert => {
            let Ok(values) = parse_exact_flags(
                arguments,
                &[
                    "--upstream",
                    "--python",
                    "--evidence",
                    "--output",
                    "--representation",
                ],
            ) else {
                return json_usage(
                    "model convert requires --upstream, --python, --evidence, --output, and --representation exactly once",
                );
            };
            let representation = match values[4].to_str() {
                Some("singleton") => ModelRepresentation::Singleton,
                Some("zero-padded-batch") => ModelRepresentation::ZeroPaddedBatch,
                Some("paired-strand-batch") => ModelRepresentation::PairedStrandBatch,
                _ => {
                    return json_usage(
                        "--representation requires singleton, zero-padded-batch, or paired-strand-batch",
                    );
                }
            };
            let arguments = ConvertArguments {
                upstream: Path::new(values[0]).to_owned(),
                python: Path::new(values[1]).to_owned(),
                evidence: Path::new(values[2]).to_owned(),
                output: Path::new(values[3]).to_owned(),
                representation,
            };
            match convert_model_bundle(&arguments) {
                Ok(value) => model_json(&value, 0),
                Err(error) => model_failure(&error),
            }
        }
        Leaf::ModelInspect => {
            let Ok(values) = parse_exact_flags(arguments, &["--bundle"]) else {
                return json_usage("model inspect requires --bundle exactly once");
            };
            match inspect_model_bundle(Path::new(values[0])) {
                Ok(value) => model_json(&value, 0),
                Err(error) => model_failure(&error),
            }
        }
        Leaf::ModelQualify => {
            let Ok(values) = parse_exact_flags(arguments, &["--bundle", "--evidence"]) else {
                return json_usage("model qualify requires --bundle and --evidence exactly once");
            };
            match qualify_model_bundle(Path::new(values[0]), Path::new(values[1])) {
                Ok(value) => model_json(&value, 0),
                Err(error) => model_failure(&error),
            }
        }
        _ => unreachable!("model dispatcher receives model leaves"),
    }
}

fn model_failure(error: &CommandError) -> ExitCode {
    let exit = u8::from(error.code == "CLI_USAGE") + 1;
    model_json(error, exit)
}

fn model_json(value: &impl serde::Serialize, exit: u8) -> ExitCode {
    let bytes = match serde_jcs::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => {
            b"{\"code\":\"IO\",\"details\":null,\"message\":\"model JSON output failed\",\"status\":\"error\"}".to_vec()
        }
    };
    let mut stream: Box<dyn std::io::Write> = if exit == 0 {
        Box::new(std::io::stdout().lock())
    } else {
        Box::new(std::io::stderr().lock())
    };
    if stream
        .write_all(&bytes)
        .and_then(|()| stream.write_all(b"\n"))
        .is_err()
    {
        return ExitCode::from(1);
    }
    ExitCode::from(exit)
}

fn reference_invalid(arguments: &[std::ffi::OsString]) -> ExitCode {
    if arguments.is_empty() {
        return reference_failure(
            "reference",
            "CLI_USAGE",
            "reference requires build, inspect, or window",
            2,
        );
    }
    reference_failure("reference", "CLI_USAGE", "reference action is invalid", 2)
}

fn reference_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::ReferenceBuild => {
            let Ok(values) = parse_exact_flags(
                arguments,
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
        Leaf::ReferenceInspect => {
            let Ok(values) = parse_exact_flags(arguments, &["--bundle"]) else {
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
        Leaf::ReferenceWindow => {
            let Ok(values) =
                parse_exact_flags(arguments, &["--bundle", "--contig", "--start", "--length"])
            else {
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
        _ => unreachable!("reference dispatcher receives reference leaves"),
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

fn compatibility_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::CompatibilityInspect => {
            let Ok(values) = parse_exact_flags(arguments, &["--corpus"]) else {
                return json_usage("compatibility inspect requires --corpus exactly once");
            };
            match inspect_corpus(Path::new(values[0])) {
                Ok(outcome) => json_success(&outcome),
                Err(error) => json_failure(&error),
            }
        }
        Leaf::CompatibilityCapture => {
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
            let Ok(values) = parse_exact_flags(arguments, &flags) else {
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
        _ => unreachable!("compatibility dispatcher receives compatibility leaves"),
    }
}

fn release_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::ReleasePrepare => {
            let Ok(values) =
                parse_exact_flags(arguments, &["--transport", "--receipt", "--output"])
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
        _ => unreachable!("release dispatcher receives release leaves"),
    }
}

fn transport_command(leaf: Leaf, arguments: &[std::ffi::OsString]) -> ExitCode {
    match leaf {
        Leaf::TransportPack => {
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
        Leaf::TransportVerify => {
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
        Leaf::TransportUnpack => {
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
        _ => unreachable!("transport dispatcher receives transport leaves"),
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
