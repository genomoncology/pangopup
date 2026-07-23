use pangopup_assets::{
    AssetError, AssetErrorKind, CachePathInputs, DataPathInputs, LocalStatus, SyncOutcome,
    install_transport, local_status, open_active_bundle, resolve_cache_root, resolve_data_root,
    sync_assets,
};
use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, ScoreProvider,
};
use pangopup_index::{BundleOpen, IndexError};
use serde::Serialize;
use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

const HELP: &str = "Pangopup: exact Pangolin score lookup\n\nUsage:\n  pangopup assets sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]\n  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]\n  pangopup assets status [--data-dir <ABSOLUTE_PATH>]\n  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table]\n  pangopup --help\n  pangopup --version";

struct Arguments {
    bundle: Option<PathBuf>,
    data_dir: Option<OsString>,
    variants: Vec<ParsedVariant>,
    gene: Option<EnsemblGeneId>,
    format: OutputFormat,
}

enum Command {
    Lookup(Arguments),
    Install {
        transport: PathBuf,
        data_dir: Option<OsString>,
    },
    Status {
        data_dir: Option<OsString>,
    },
    Sync {
        offline: bool,
        data_dir: Option<OsString>,
        cache_dir: Option<OsString>,
    },
}

struct ParsedVariant {
    contig: String,
    position: u32,
    reference: DnaBase,
    alternate: DnaBase,
}

#[derive(Serialize)]
struct ErrorLine<'a> {
    status: &'static str,
    code: &'a str,
    message: &'a str,
    details: Option<()>,
}

#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: String,
    exit: u8,
}

type SyncRunner = dyn Fn(&Path, Option<&Path>, bool) -> Result<SyncOutcome, AssetError>;

impl Failure {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            code: "CLI_USAGE",
            message: message.into(),
            exit: 2,
        }
    }
    fn variant(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_VARIANT",
            message: message.into(),
            exit: 2,
        }
    }
    fn gene(message: impl Into<String>) -> Self {
        Self {
            code: "INVALID_GENE",
            message: message.into(),
            exit: 2,
        }
    }
}

fn main() -> ExitCode {
    let raw: Vec<OsString> = std::env::args_os().skip(1).collect();
    match raw.as_slice() {
        [] => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        [value] if value == "-h" || value == "--help" => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        [value] if value == "-V" || value == "--version" => {
            println!("pangopup {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        [command, value] if command == "lookup" && (value == "-h" || value == "--help") => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        [command, value] if command == "lookup" && (value == "-V" || value == "--version") => {
            println!("pangopup {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    match run(&raw) {
        Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => fail(&Failure {
                code: "OUTPUT_IO",
                message: error.to_string(),
                exit: 1,
            }),
        },
        Err(error) => fail(&error),
    }
}

fn run(raw: &[OsString]) -> Result<Vec<u8>, Failure> {
    run_with_sync(raw, &|data, cache, offline| {
        sync_assets(data, cache, offline)
    })
}

fn run_with_sync(raw: &[OsString], syncer: &SyncRunner) -> Result<Vec<u8>, Failure> {
    match parse_command(raw)? {
        Command::Lookup(arguments) => run_lookup(arguments),
        Command::Install {
            transport,
            data_dir,
        } => {
            let root = data_root(data_dir)?;
            let result = install_transport(&transport, &root).map_err(map_install_error)?;
            json_line(&result)
        }
        Command::Status { data_dir } => {
            let root = data_root(data_dir)?;
            let status = local_status(&root).map_err(map_status_error)?;
            match status {
                LocalStatus::Missing { data_dir } => json_line(&MissingStatus {
                    status: "missing",
                    data_dir,
                }),
                LocalStatus::Installing { data_dir } => json_line(&MissingStatus {
                    status: "installing",
                    data_dir,
                }),
                LocalStatus::Ready { active, installing } => json_line(&ReadyStatus {
                    status: "ready",
                    bundle_id: active.bundle_id,
                    transport_id: active.transport_id,
                    path: active.path,
                    installing,
                }),
            }
        }
        Command::Sync {
            offline,
            data_dir,
            cache_dir,
        } => {
            let cache = resolve_cache_root(&CachePathInputs::from_environment(cache_dir))
                .map_err(map_path_error)?;
            let root = data_root(data_dir)?;
            let result = syncer(&root, cache.as_deref(), offline).map_err(map_sync_error)?;
            json_line(&result)
        }
    }
}

#[derive(Serialize)]
struct MissingStatus {
    status: &'static str,
    data_dir: PathBuf,
}

#[derive(Serialize)]
struct ReadyStatus {
    status: &'static str,
    bundle_id: String,
    transport_id: String,
    path: PathBuf,
    installing: bool,
}

fn json_line(value: &impl Serialize) -> Result<Vec<u8>, Failure> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| Failure {
        code: "OUTPUT_IO",
        message: error.to_string(),
        exit: 1,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn data_root(explicit: Option<OsString>) -> Result<PathBuf, Failure> {
    resolve_data_root(&DataPathInputs::from_environment(explicit)).map_err(map_path_error)
}

fn run_lookup(arguments: Arguments) -> Result<Vec<u8>, Failure> {
    let bundle = match arguments.bundle {
        Some(path) => BundleOpen::open(&path).map_err(map_open_error)?,
        None => {
            let root = data_root(arguments.data_dir)?;
            open_active_bundle(&root).map_err(map_lookup_asset_error)?.1
        }
    };
    let mut requests = Vec::with_capacity(arguments.variants.len());
    for parsed in arguments.variants {
        let (contig, length) = bundle.resolve_contig(&parsed.contig).ok_or_else(|| {
            Failure::variant(format!("unsupported GRCh38 contig {}", parsed.contig))
        })?;
        if parsed.position > length {
            return Err(Failure::variant(format!(
                "position {} exceeds {} length {}",
                parsed.position, contig, length
            )));
        }
        let snv = Grch38Snv::new(
            contig,
            GenomicPosition::new(parsed.position)
                .map_err(|error| Failure::variant(error.to_string()))?,
            parsed.reference,
            parsed.alternate,
        )
        .map_err(|error| Failure::variant(error.to_string()))?;
        let result = bundle
            .lookup(snv, arguments.gene)
            .map_err(|error| Failure {
                code: "LOOKUP_CORRUPT",
                message: error.to_string(),
                exit: 1,
            })?;
        requests.push(RenderRequest::new(snv, result));
    }
    render_requests(arguments.format, &requests).map_err(|error| Failure {
        code: "LOOKUP_CORRUPT",
        message: error.to_string(),
        exit: 1,
    })
}

fn parse_command(raw: &[OsString]) -> Result<Command, Failure> {
    match raw.first().and_then(|value| value.to_str()) {
        Some("lookup") => parse_lookup(raw).map(Command::Lookup),
        Some("assets") => parse_assets(raw),
        _ => Err(Failure::usage(HELP)),
    }
}

fn parse_assets(raw: &[OsString]) -> Result<Command, Failure> {
    let action = raw
        .get(1)
        .and_then(|value| value.to_str())
        .ok_or_else(|| Failure::usage("assets requires sync, install, or status"))?;
    let mut data_dir = None;
    let mut cache_dir = None;
    let mut transport = None;
    let mut offline = false;
    let mut index = 2;
    while index < raw.len() {
        let option = raw[index]
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        index += 1;
        if option == "--offline" && action == "sync" {
            if offline {
                return Err(Failure::usage("--offline may be supplied once"));
            }
            offline = true;
            continue;
        }
        let value = raw
            .get(index)
            .ok_or_else(|| Failure::usage(format!("{option} requires a value")))?;
        match option {
            "--data-dir" => {
                if data_dir.replace(value.clone()).is_some() {
                    return Err(Failure::usage("--data-dir may be supplied once"));
                }
            }
            "--transport" if action == "install" => {
                if transport.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--transport may be supplied once"));
                }
            }
            "--cache-dir" if action == "sync" => {
                if cache_dir.replace(value.clone()).is_some() {
                    return Err(Failure::usage("--cache-dir may be supplied once"));
                }
            }
            _ => return Err(Failure::usage(format!("unknown assets option {option}"))),
        }
        index += 1;
    }
    match action {
        "install" => Ok(Command::Install {
            transport: transport
                .ok_or_else(|| Failure::usage("assets install requires --transport"))?,
            data_dir,
        }),
        "status" if transport.is_none() => Ok(Command::Status { data_dir }),
        "sync" if transport.is_none() => Ok(Command::Sync {
            offline,
            data_dir,
            cache_dir,
        }),
        _ => Err(Failure::usage("assets requires sync, install, or status")),
    }
}

fn parse_lookup(raw: &[OsString]) -> Result<Arguments, Failure> {
    let mut bundle = None;
    let mut data_dir = None;
    let mut variants = Vec::new();
    let mut gene = None;
    let mut format = OutputFormat::Jsonl;
    let mut seen_format = false;
    let mut index = 1;
    while index < raw.len() {
        let option = raw[index]
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        index += 1;
        let value = raw
            .get(index)
            .ok_or_else(|| Failure::usage(format!("{option} requires a value")))?;
        match option {
            "--bundle" => {
                if bundle.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--bundle may be supplied once"));
                }
            }
            "--data-dir" => {
                if data_dir.replace(value.clone()).is_some() {
                    return Err(Failure::usage("--data-dir may be supplied once"));
                }
            }
            "--variant" => variants.push(parse_variant(utf8_argument(value)?)?),
            "--gene" => {
                let parsed = EnsemblGeneId::from_str(utf8_argument(value)?)
                    .map_err(|error| Failure::gene(error.to_string()))?;
                if gene.replace(parsed).is_some() {
                    return Err(Failure::usage("--gene may be supplied once"));
                }
            }
            "--format" => {
                if seen_format {
                    return Err(Failure::usage("--format may be supplied once"));
                }
                seen_format = true;
                format = match utf8_argument(value)? {
                    "jsonl" => OutputFormat::Jsonl,
                    "table" => OutputFormat::Table,
                    _ => return Err(Failure::usage("--format must be jsonl or table")),
                };
            }
            _ => return Err(Failure::usage(format!("unknown lookup option {option}"))),
        }
        index += 1;
    }
    if bundle.is_some() && data_dir.is_some() {
        return Err(Failure::usage(
            "--bundle and --data-dir are mutually exclusive",
        ));
    }
    if variants.is_empty() {
        return Err(Failure::usage("lookup requires at least one --variant"));
    }
    Ok(Arguments {
        bundle,
        data_dir,
        variants,
        gene,
        format,
    })
}

fn utf8_argument(value: &OsString) -> Result<&str, Failure> {
    value
        .to_str()
        .ok_or_else(|| Failure::usage("arguments must be UTF-8"))
}

fn map_path_error(error: AssetError) -> Failure {
    Failure {
        code: error.kind().code(),
        message: error.to_string(),
        exit: 2,
    }
}

fn map_install_error(error: AssetError) -> Failure {
    Failure {
        code: error.kind().code(),
        message: error.to_string(),
        exit: if matches!(
            error.kind(),
            AssetErrorKind::PathInvalid | AssetErrorKind::PathUnavailable
        ) {
            2
        } else {
            1
        },
    }
}

fn map_status_error(error: AssetError) -> Failure {
    let code = match error.kind() {
        AssetErrorKind::InstallConflict => "BUNDLE_INCOMPATIBLE",
        _ => error.kind().code(),
    };
    Failure {
        code,
        message: error.to_string(),
        exit: 1,
    }
}

fn map_sync_error(error: AssetError) -> Failure {
    Failure {
        code: error.kind().code(),
        message: error.to_string(),
        exit: if matches!(
            error.kind(),
            AssetErrorKind::PathInvalid | AssetErrorKind::PathUnavailable
        ) {
            2
        } else {
            1
        },
    }
}

fn map_lookup_asset_error(error: AssetError) -> Failure {
    let code = match error.kind() {
        AssetErrorKind::InstallConflict => "BUNDLE_INCOMPATIBLE",
        _ => error.kind().code(),
    };
    Failure {
        code,
        message: error.to_string(),
        exit: 1,
    }
}

fn parse_variant(value: &str) -> Result<ParsedVariant, Failure> {
    let mut fields = value.split(':');
    let (Some(assembly), Some(contig), Some(position), Some(reference), Some(alternate), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(Failure::variant(
            "variant must be GRCh38:CONTIG:POS:REF:ALT",
        ));
    };
    if assembly != "GRCh38" {
        return Err(Failure::variant("assembly must be GRCh38"));
    }
    if !valid_contig_syntax(contig) {
        return Err(Failure::variant("invalid contig spelling"));
    }
    let position = position
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| position.parse::<u32>().ok())
        .flatten()
        .filter(|value| *value != 0)
        .ok_or_else(|| Failure::variant("position must be a nonzero decimal u32"))?;
    let reference =
        DnaBase::parse(reference).map_err(|error| Failure::variant(error.to_string()))?;
    let alternate =
        DnaBase::parse(alternate).map_err(|error| Failure::variant(error.to_string()))?;
    if reference == alternate {
        return Err(Failure::variant(
            "reference and alternate bases must differ",
        ));
    }
    Ok(ParsedVariant {
        contig: contig.to_owned(),
        position,
        reference,
        alternate,
    })
}

fn valid_contig_syntax(value: &str) -> bool {
    value.parse::<Grch38Contig>().is_ok()
        || matches!(
            value,
            "NC_000001.11"
                | "NC_000002.12"
                | "NC_000003.12"
                | "NC_000004.12"
                | "NC_000005.10"
                | "NC_000006.12"
                | "NC_000007.14"
                | "NC_000008.11"
                | "NC_000009.12"
                | "NC_000010.11"
                | "NC_000011.10"
                | "NC_000012.12"
                | "NC_000013.11"
                | "NC_000014.9"
                | "NC_000015.10"
                | "NC_000016.10"
                | "NC_000017.11"
                | "NC_000018.10"
                | "NC_000019.10"
                | "NC_000020.11"
                | "NC_000021.9"
                | "NC_000022.11"
                | "NC_000023.11"
                | "NC_000024.10"
                | "NC_012920.1"
        )
}

fn map_open_error(error: IndexError) -> Failure {
    let code = match error {
        IndexError::Io(_) => "BUNDLE_IO",
        IndexError::Incompatible(_) => "BUNDLE_INCOMPATIBLE",
        _ => "BUNDLE_INVALID",
    };
    Failure {
        code,
        message: error.to_string(),
        exit: 1,
    }
}

fn fail(error: &Failure) -> ExitCode {
    let line = ErrorLine {
        status: "error",
        code: error.code,
        message: &error.message,
        details: None,
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &line);
    let _ = stderr.write_all(b"\n");
    ExitCode::from(error.exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injected_sync_adapter_renders_exact_compact_json() {
        let args = [
            OsString::from("assets"),
            OsString::from("sync"),
            OsString::from("--offline"),
            OsString::from("--data-dir"),
            OsString::from("/tmp/pangopup-sync-data"),
            OsString::from("--cache-dir"),
            OsString::from("/tmp/pangopup-sync-cache"),
        ];
        let bytes = run_with_sync(&args, &|data, cache, offline| {
            assert_eq!(data, Path::new("/tmp/pangopup-sync-data"));
            assert_eq!(cache, Some(Path::new("/tmp/pangopup-sync-cache")));
            assert!(offline);
            Ok(SyncOutcome {
                status: "installed",
                profile: "snv-grch38-v1".to_owned(),
                bundle_id: "sha256:bundle".to_owned(),
                transport_id: "sha256:transport".to_owned(),
                path: PathBuf::from("/tmp/pangopup-sync-data/bundles/bundle/bundle"),
                downloaded_bytes: 123,
                resumed_bytes: 45,
            })
        })
        .expect("sync output");
        assert_eq!(
            bytes,
            b"{\"status\":\"installed\",\"profile\":\"snv-grch38-v1\",\"bundle_id\":\"sha256:bundle\",\"transport_id\":\"sha256:transport\",\"path\":\"/tmp/pangopup-sync-data/bundles/bundle/bundle\",\"downloaded_bytes\":123,\"resumed_bytes\":45}\n"
        );
    }

    #[test]
    fn sync_grammar_rejects_duplicates_and_values_for_flags() {
        for args in [
            vec!["assets", "sync", "--offline", "--offline"],
            vec![
                "assets",
                "sync",
                "--cache-dir",
                "/tmp/a",
                "--cache-dir",
                "/tmp/b",
            ],
            vec!["assets", "status", "--offline"],
        ] {
            let raw: Vec<OsString> = args.into_iter().map(OsString::from).collect();
            assert!(parse_command(&raw).is_err());
        }
    }
}
