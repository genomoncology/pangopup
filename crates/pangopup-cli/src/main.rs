use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, ScoreProvider,
};
use pangopup_index::{BundleOpen, IndexError};
use serde::Serialize;
use std::{ffi::OsString, io::Write, path::PathBuf, process::ExitCode, str::FromStr};

const HELP: &str = "Pangopup: exact Pangolin score lookup\n\nUsage:\n  pangopup lookup --bundle <PATH> --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table]\n  pangopup --help\n  pangopup --version";

struct Arguments {
    bundle: PathBuf,
    variants: Vec<ParsedVariant>,
    gene: Option<EnsemblGeneId>,
    format: OutputFormat,
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
    let arguments = parse_arguments(raw)?;
    let bundle = BundleOpen::open(&arguments.bundle).map_err(map_open_error)?;
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

fn parse_arguments(raw: &[OsString]) -> Result<Arguments, Failure> {
    if raw.first().is_none_or(|value| value != "lookup") {
        return Err(Failure::usage(HELP));
    }
    let mut bundle = None;
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
        let text = value
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        match option {
            "--bundle" => {
                if bundle.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--bundle may be supplied once"));
                }
            }
            "--variant" => variants.push(parse_variant(text)?),
            "--gene" => {
                let parsed = EnsemblGeneId::from_str(text)
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
                format = match text {
                    "jsonl" => OutputFormat::Jsonl,
                    "table" => OutputFormat::Table,
                    _ => return Err(Failure::usage("--format must be jsonl or table")),
                };
            }
            _ => return Err(Failure::usage(format!("unknown lookup option {option}"))),
        }
        index += 1;
    }
    let bundle = bundle.ok_or_else(|| Failure::usage("lookup requires --bundle"))?;
    if variants.is_empty() {
        return Err(Failure::usage("lookup requires at least one --variant"));
    }
    Ok(Arguments {
        bundle,
        variants,
        gene,
        format,
    })
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
