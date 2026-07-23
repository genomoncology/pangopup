use pangopup_assets::{
    AssetError, AssetErrorKind, DataPathInputs, LocalStatus, install_transport, local_status,
    open_active_bundle, resolve_data_root,
};
use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, ScoreProvider,
};
use pangopup_index::{BundleOpen, IndexError};
use serde::Serialize;
use std::{ffi::OsString, io::Write, path::PathBuf, process::ExitCode, str::FromStr};

const HELP: &str = "Pangopup: exact Pangolin score lookup\n\nUsage:\n  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]\n  pangopup assets status [--data-dir <ABSOLUTE_PATH>]\n  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table]\n  pangopup --help\n  pangopup --version";

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

struct Failure {
    code: &'static str,
    message: String,
    exit: u8,
}

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
        .ok_or_else(|| Failure::usage("assets requires install or status"))?;
    let mut data_dir = None;
    let mut transport = None;
    let mut index = 2;
    while index < raw.len() {
        let option = raw[index]
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        index += 1;
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
        _ => Err(Failure::usage("assets requires install or status")),
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
