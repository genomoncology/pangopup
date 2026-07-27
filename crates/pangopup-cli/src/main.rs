use pangopup_assets::{
    AssetError, AssetErrorKind, CachePathInputs, DataPathInputs, LocalStatus, RuntimeLocalStatus,
    SyncOutcome, install_runtime_profile, install_transport, local_status, open_active_bundle,
    resolve_cache_root, resolve_data_root, runtime_local_status, sync_assets,
};
use pangopup_cache::{CacheIdentity, CacheKey, EntryLimit, ModelResultCache};
use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, Grch38Snv, Grch38Variant,
    ReferenceProvenance, ScoreProvider,
};
use pangopup_engine::{
    LookupFirstRouter, ModelFallback, ModelFallbackError, ModelProvenance, RouteDecision,
    RouteRequest, RoutedResult,
};
use pangopup_index::{
    BundleOpen, IndexError,
    mask::{AdmittedMaskDomains, MaskDomainsOpen},
    reference::{ReferenceBundleOpen, required_accession},
    reference_admission::{ReferenceAdmission, inspect_reference_admission},
};
use pangopup_model::{CpuPolicy, ModelAdmission, ModelKernel, inspect_model_admission};
use serde::Serialize;
use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

const HELP: &str = "Pangopup: exact Pangolin score lookup\n\nUsage:\n  pangopup assets sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]\n  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]\n  pangopup assets status [--data-dir <ABSOLUTE_PATH>]\n  pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]\n  pangopup assets runtime status [--data-dir <ABSOLUTE_PATH>]\n  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]\n  pangopup --help\n  pangopup --version";

struct Arguments {
    bundle: Option<PathBuf>,
    data_dir: Option<OsString>,
    variants: Vec<Grch38Variant>,
    gene: Option<EnsemblGeneId>,
    format: OutputFormat,
    fallback: Option<FallbackPaths>,
    cache: Option<CacheOptions>,
}

struct FallbackPaths {
    model: PathBuf,
    reference: PathBuf,
    mask: PathBuf,
}

struct CacheOptions {
    path: PathBuf,
    limit: EntryLimit,
    disposable_default: bool,
}

struct FallbackAdmission {
    model: ModelAdmission,
    reference: ReferenceAdmission,
    mask: Option<AdmittedMaskDomains>,
    provenance: ModelProvenance,
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
    RuntimeInstall {
        profile: PathBuf,
        model_bundle: PathBuf,
        reference_bundle: PathBuf,
        mask: PathBuf,
        data_dir: Option<OsString>,
    },
    RuntimeStatus {
        data_dir: Option<OsString>,
    },
    Sync {
        offline: bool,
        data_dir: Option<OsString>,
        cache_dir: Option<OsString>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FallbackComponent {
    Reference,
    Mask,
    Model,
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

    fn model_assets_required() -> Self {
        Self {
            code: "MODEL_ASSETS_REQUIRED",
            message: "model scoring requires --model-bundle, --reference-bundle, and --mask"
                .to_owned(),
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
        Command::RuntimeInstall {
            profile,
            model_bundle,
            reference_bundle,
            mask,
            data_dir,
        } => {
            let root = data_root(data_dir)?;
            let result =
                install_runtime_profile(&profile, &model_bundle, &reference_bundle, &mask, &root)
                    .map_err(map_runtime_error)?;
            json_line(&result)
        }
        Command::RuntimeStatus { data_dir } => {
            let root = data_root(data_dir)?;
            match runtime_local_status(&root).map_err(map_runtime_error)? {
                RuntimeLocalStatus::Missing { data_dir } => json_line(&MissingStatus {
                    status: "missing",
                    data_dir,
                }),
                RuntimeLocalStatus::Installing { data_dir } => json_line(&MissingStatus {
                    status: "installing",
                    data_dir,
                }),
                RuntimeLocalStatus::Ready {
                    profile_id,
                    snv_bundle_id,
                    model_bundle_id,
                    reference_bundle_id,
                    mask_sha256,
                    model_path,
                    reference_path,
                    mask_path,
                    installing,
                } => json_line(&RuntimeReadyStatus {
                    status: "ready",
                    profile_id,
                    snv_bundle_id,
                    model_bundle_id,
                    reference_bundle_id,
                    mask_sha256,
                    model_path,
                    reference_path,
                    mask_path,
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
struct RuntimeReadyStatus {
    status: &'static str,
    profile_id: String,
    snv_bundle_id: String,
    model_bundle_id: String,
    reference_bundle_id: String,
    mask_sha256: String,
    model_path: PathBuf,
    reference_path: PathBuf,
    mask_path: PathBuf,
    installing: bool,
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
    run_lookup_with_observer(arguments, &mut |_| {})
}

fn run_lookup_with_observer(
    arguments: Arguments,
    observer: &mut dyn FnMut(FallbackComponent),
) -> Result<Vec<u8>, Failure> {
    if arguments.fallback.is_none()
        && arguments
            .variants
            .iter()
            .any(|variant| snv_from_variant(variant).is_none())
    {
        return Err(Failure::model_assets_required());
    }
    let Arguments {
        bundle: bundle_path,
        data_dir,
        variants,
        gene,
        format,
        fallback: fallback_paths,
        cache: cache_options,
    } = arguments;
    let bundle = match bundle_path {
        Some(path) => BundleOpen::open(&path).map_err(map_open_error)?,
        None => {
            let root = data_root(data_dir)?;
            open_active_bundle(&root).map_err(map_lookup_asset_error)?.1
        }
    };
    // Preserve the original lookup-only contract when model fallback was not
    // explicitly enabled. In particular, a pure SNV miss remains a rendered
    // precomputed `not_found`; only a non-SNV requires model assets.
    if fallback_paths.is_none() {
        let mut requests = Vec::with_capacity(variants.len());
        for variant in variants {
            let snv = snv_from_variant(&variant).ok_or_else(Failure::model_assets_required)?;
            let (_, length) = bundle
                .resolve_contig(&variant.contig().to_string())
                .ok_or_else(|| {
                    Failure::variant(format!("unsupported GRCh38 contig {}", variant.contig()))
                })?;
            if variant.position().get() > length {
                return Err(Failure::variant(format!(
                    "position {} exceeds {} length {}",
                    variant.position(),
                    variant.contig(),
                    length
                )));
            }
            let result = bundle.lookup(snv, gene).map_err(map_lookup_error)?;
            requests.push(RenderRequest::new(snv, result));
        }
        return render_lookup_requests(format, &requests);
    }

    let router = LookupFirstRouter::new(bundle);
    let mut decisions = Vec::with_capacity(variants.len());
    let mut needs_model = false;
    for variant in variants {
        let decision = router
            .inspect(RouteRequest::new(variant, gene))
            .map_err(map_lookup_error)?;
        needs_model |= matches!(decision, RouteDecision::ModelRequired(_));
        decisions.push(decision);
    }

    let mut admission = if needs_model {
        Some(admit_model_fallback(
            fallback_paths
                .as_ref()
                .expect("complete fallback set was parsed"),
        )?)
    } else {
        None
    };
    let mut cache = if needs_model {
        let options = cache_options
            .as_ref()
            .expect("fallback parsing always resolves cache options");
        let result = if options.disposable_default {
            ModelResultCache::open_default(&options.path, options.limit)
        } else {
            ModelResultCache::open_explicit(&options.path, options.limit)
        };
        match result {
            Ok(cache) => Some(cache),
            Err(pangopup_cache::CacheError::Busy) => None,
            Err(error) => return Err(map_cache_error(error)),
        }
    } else {
        None
    };
    let mut fallback = None;
    let mut requests = Vec::with_capacity(decisions.len());
    for decision in decisions {
        let routed = match decision {
            RouteDecision::Authoritative(result) => result,
            RouteDecision::ModelRequired(required) => {
                let (key, cached_provenance) = {
                    let admitted = admission
                        .as_ref()
                        .expect("model-required batch has one admission");
                    (
                        cache_key(required.variant(), admitted).map_err(map_cache_error)?,
                        admitted.provenance.clone(),
                    )
                };
                let cached = match cache.as_mut() {
                    Some(cache) => match cache.get(&key) {
                        Ok(value) => value,
                        Err(pangopup_cache::CacheError::Busy) => None,
                        Err(error) => return Err(map_cache_error(error)),
                    },
                    None => None,
                };
                if let Some(records) = cached {
                    routed_from_cached(required, records, cached_provenance)
                } else {
                    if fallback.is_none() {
                        fallback = Some(open_admitted_model_fallback(
                            fallback_paths
                                .as_ref()
                                .expect("complete fallback set was parsed"),
                            admission
                                .as_mut()
                                .expect("model-required batch has one admission"),
                            observer,
                        )?);
                    }
                    let filter = required.gene();
                    let modeled = fallback
                        .as_mut()
                        .expect("model miss opened one fallback")
                        .complete_unfiltered(required)
                        .map_err(map_model_fallback_error)?;
                    if let RoutedResult::Modeled {
                        variant,
                        mut records,
                        provenance,
                    } = modeled
                    {
                        // Cache failure cannot invalidate a successful model answer.
                        if let Some(cache) = cache.as_mut() {
                            let _cache_write = cache.put(&key, &records);
                        }
                        if let Some(filter) = filter {
                            records.retain(|record| record.gene().stable() == filter);
                        }
                        RoutedResult::Modeled {
                            variant,
                            records,
                            provenance,
                        }
                    } else {
                        unreachable!("model fallback always returns modeled output")
                    }
                }
            }
        };
        requests.push(RenderRequest::from_routed(routed));
    }
    render_lookup_requests(format, &requests)
}

fn routed_from_cached(
    required: pangopup_engine::ModelRequired,
    mut records: Vec<pangopup_core::ModelGeneScoreRecord>,
    provenance: ModelProvenance,
) -> RoutedResult {
    if let Some(filter) = required.gene() {
        records.retain(|record| record.gene().stable() == filter);
    }
    RoutedResult::Modeled {
        variant: required.variant().clone(),
        records,
        provenance,
    }
}

fn cache_key(
    variant: &Grch38Variant,
    admission: &FallbackAdmission,
) -> Result<CacheKey, pangopup_cache::CacheError> {
    let identity = CacheIdentity::new(
        &admission.model.bundle_id().to_string(),
        admission.model.profile(),
        &admission.model.representation().to_string(),
        &CpuPolicy::production_default().to_string(),
        admission.reference.bundle_id(),
        admission.reference.profile(),
        admission.reference.sequence_set_sha256(),
        admission.provenance.mask_bytes(),
        admission.provenance.mask_sha256(),
    )?;
    Ok(CacheKey::new(variant, identity))
}

fn snv_from_variant(variant: &Grch38Variant) -> Option<Grch38Snv> {
    if variant.reference().len() != 1 || variant.alternate().len() != 1 {
        return None;
    }
    let reference =
        DnaBase::parse(variant.reference()).expect("one-base variant has one uppercase DNA base");
    let alternate =
        DnaBase::parse(variant.alternate()).expect("one-base variant has one uppercase DNA base");
    Some(
        Grch38Snv::new(variant.contig(), variant.position(), reference, alternate)
            .expect("variant construction rejects equal alleles"),
    )
}

fn map_lookup_error(error: pangopup_core::LookupError) -> Failure {
    Failure {
        code: "LOOKUP_CORRUPT",
        message: error.to_string(),
        exit: 1,
    }
}

fn render_lookup_requests(
    format: OutputFormat,
    requests: &[RenderRequest],
) -> Result<Vec<u8>, Failure> {
    render_requests(format, requests).map_err(|error| Failure {
        code: "LOOKUP_CORRUPT",
        message: error.to_string(),
        exit: 1,
    })
}

#[cfg(test)]
fn open_model_fallback(
    paths: &FallbackPaths,
    observer: &mut dyn FnMut(FallbackComponent),
) -> Result<ModelFallback, Failure> {
    observer(FallbackComponent::Reference);
    let reference =
        ReferenceBundleOpen::open_identified(&paths.reference).map_err(|_| Failure {
            code: "REFERENCE_BUNDLE_INVALID",
            message: "reference bundle is invalid".to_owned(),
            exit: 1,
        })?;
    observer(FallbackComponent::Mask);
    let mask = MaskDomainsOpen::open_identified(&paths.mask).map_err(|_| Failure {
        code: "MASK_INVALID",
        message: "mask member is invalid".to_owned(),
        exit: 1,
    })?;
    observer(FallbackComponent::Model);
    let model = ModelKernel::open(&paths.model).map_err(|_| Failure {
        code: "MODEL_BUNDLE_INVALID",
        message: "model bundle is invalid".to_owned(),
        exit: 1,
    })?;
    Ok(ModelFallback::new(reference, mask, model))
}

fn admit_model_fallback(paths: &FallbackPaths) -> Result<FallbackAdmission, Failure> {
    let reference = inspect_reference_admission(&paths.reference).map_err(|_| Failure {
        code: "REFERENCE_BUNDLE_INVALID",
        message: "reference bundle is invalid".to_owned(),
        exit: 1,
    })?;
    let mask = MaskDomainsOpen::admit(&paths.mask).map_err(|_| Failure {
        code: "MASK_INVALID",
        message: "mask member is invalid".to_owned(),
        exit: 1,
    })?;
    let model = inspect_model_admission(&paths.model).map_err(|_| Failure {
        code: "MODEL_BUNDLE_INVALID",
        message: "model bundle is invalid".to_owned(),
        exit: 1,
    })?;
    let provenance = ModelProvenance::new(
        model.bundle_id().to_string(),
        model.profile().to_owned(),
        ReferenceProvenance::new(
            reference.bundle_id().to_owned(),
            reference.profile().to_owned(),
            reference.format().to_owned(),
            reference.assembly().to_owned(),
            reference.assembly_accession().to_owned(),
            reference.sequence_set_sha256().to_owned(),
        ),
        mask.identity().bytes(),
        format!("sha256:{}", mask.identity().sha256()),
    );
    Ok(FallbackAdmission {
        model,
        reference,
        mask: Some(mask),
        provenance,
    })
}

fn open_admitted_model_fallback(
    paths: &FallbackPaths,
    admission: &mut FallbackAdmission,
    observer: &mut dyn FnMut(FallbackComponent),
) -> Result<ModelFallback, Failure> {
    observer(FallbackComponent::Reference);
    let reference =
        ReferenceBundleOpen::open_identified(&paths.reference).map_err(|_| Failure {
            code: "REFERENCE_BUNDLE_INVALID",
            message: "reference bundle is invalid".to_owned(),
            exit: 1,
        })?;
    observer(FallbackComponent::Mask);
    let mask = admission
        .mask
        .take()
        .expect("admitted mask is consumed only for first model miss")
        .open()
        .map_err(|_| Failure {
            code: "MASK_INVALID",
            message: "mask member is invalid".to_owned(),
            exit: 1,
        })?;
    observer(FallbackComponent::Model);
    let model = ModelKernel::open(&paths.model).map_err(|_| Failure {
        code: "MODEL_BUNDLE_INVALID",
        message: "model bundle is invalid".to_owned(),
        exit: 1,
    })?;
    let fallback = ModelFallback::new(reference, mask, model);
    if fallback.provenance() != &admission.provenance {
        return Err(Failure {
            code: "MODEL_ASSET_IDENTITY_CHANGED",
            message: "model assets changed after cache admission".to_owned(),
            exit: 1,
        });
    }
    Ok(fallback)
}

fn map_cache_error(error: pangopup_cache::CacheError) -> Failure {
    Failure {
        code: "MODEL_CACHE_INVALID",
        message: error.to_string(),
        exit: 1,
    }
}

fn map_model_fallback_error(error: ModelFallbackError) -> Failure {
    match error {
        ModelFallbackError::Rejected(error) => Failure {
            code: "MODEL_REJECTED",
            message: error.to_string(),
            exit: 2,
        },
        ModelFallbackError::Scoring(error) => Failure {
            code: "MODEL_SCORING",
            message: error.to_string(),
            exit: 1,
        },
    }
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
    if action == "runtime" {
        return parse_runtime_assets(raw);
    }
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

fn parse_runtime_assets(raw: &[OsString]) -> Result<Command, Failure> {
    let action = raw
        .get(2)
        .and_then(|value| value.to_str())
        .ok_or_else(|| Failure::usage("assets runtime requires install or status"))?;
    let mut data_dir = None;
    let mut profile = None;
    let mut model_bundle = None;
    let mut reference_bundle = None;
    let mut mask = None;
    let mut index = 3;
    while index < raw.len() {
        let option = raw[index]
            .to_str()
            .ok_or_else(|| Failure::usage("arguments must be UTF-8"))?;
        index += 1;
        let value = raw
            .get(index)
            .ok_or_else(|| Failure::usage(format!("{option} requires a value")))?;
        let slot = match option {
            "--data-dir" => &mut data_dir,
            "--profile" if action == "install" => &mut profile,
            "--model-bundle" if action == "install" => &mut model_bundle,
            "--reference-bundle" if action == "install" => &mut reference_bundle,
            "--mask" if action == "install" => &mut mask,
            _ => {
                return Err(Failure::usage(format!(
                    "unknown assets runtime option {option}"
                )));
            }
        };
        if slot.replace(value.clone()).is_some() {
            return Err(Failure::usage(format!("{option} may be supplied once")));
        }
        index += 1;
    }
    match action {
        "install" => Ok(Command::RuntimeInstall {
            profile: PathBuf::from(
                profile
                    .ok_or_else(|| Failure::usage("assets runtime install requires --profile"))?,
            ),
            model_bundle: PathBuf::from(
                model_bundle.ok_or_else(|| {
                    Failure::usage("assets runtime install requires --model-bundle")
                })?,
            ),
            reference_bundle: PathBuf::from(reference_bundle.ok_or_else(|| {
                Failure::usage("assets runtime install requires --reference-bundle")
            })?),
            mask: PathBuf::from(
                mask.ok_or_else(|| Failure::usage("assets runtime install requires --mask"))?,
            ),
            data_dir,
        }),
        "status"
            if profile.is_none()
                && model_bundle.is_none()
                && reference_bundle.is_none()
                && mask.is_none() =>
        {
            Ok(Command::RuntimeStatus { data_dir })
        }
        _ => Err(Failure::usage("assets runtime requires install or status")),
    }
}

fn parse_lookup(raw: &[OsString]) -> Result<Arguments, Failure> {
    let mut bundle = None;
    let mut data_dir = None;
    let mut variants = Vec::new();
    let mut gene = None;
    let mut format = OutputFormat::Jsonl;
    let mut model_bundle = None;
    let mut reference_bundle = None;
    let mut mask = None;
    let mut model_cache = None;
    let mut model_cache_max_entries = None;
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
            "--model-bundle" => {
                if model_bundle.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--model-bundle may be supplied once"));
                }
            }
            "--reference-bundle" => {
                if reference_bundle.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--reference-bundle may be supplied once"));
                }
            }
            "--mask" => {
                if mask.replace(PathBuf::from(value)).is_some() {
                    return Err(Failure::usage("--mask may be supplied once"));
                }
            }
            "--model-cache" => {
                let path = PathBuf::from(value);
                validate_model_cache_path(&path)?;
                if model_cache.replace(path).is_some() {
                    return Err(Failure::usage("--model-cache may be supplied once"));
                }
            }
            "--model-cache-max-entries" => {
                let parsed = EntryLimit::from_str(utf8_argument(value)?)
                    .map_err(|error| Failure::usage(error.to_string()))?;
                if model_cache_max_entries.replace(parsed).is_some() {
                    return Err(Failure::usage(
                        "--model-cache-max-entries may be supplied once",
                    ));
                }
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
    let fallback = match (model_bundle, reference_bundle, mask) {
        (None, None, None) => None,
        (Some(model), Some(reference), Some(mask)) => Some(FallbackPaths {
            model,
            reference,
            mask,
        }),
        _ => {
            return Err(Failure::usage(
                "--model-bundle, --reference-bundle, and --mask must be supplied together",
            ));
        }
    };
    if fallback.is_none() && (model_cache.is_some() || model_cache_max_entries.is_some()) {
        return Err(Failure::usage(
            "model cache options require --model-bundle, --reference-bundle, and --mask",
        ));
    }
    let cache = fallback
        .as_ref()
        .map(|_| resolve_model_cache_options(model_cache, model_cache_max_entries))
        .transpose()?;
    Ok(Arguments {
        bundle,
        data_dir,
        variants,
        gene,
        format,
        fallback,
        cache,
    })
}

fn validate_model_cache_path(path: &Path) -> Result<(), Failure> {
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(Failure::usage(
            "model cache path must be an absolute file path",
        ));
    }
    Ok(())
}

fn resolve_model_cache_options(
    cli_path: Option<PathBuf>,
    cli_limit: Option<EntryLimit>,
) -> Result<CacheOptions, Failure> {
    let environment_path = std::env::var_os("PANGOPUP_MODEL_CACHE").map(PathBuf::from);
    if let Some(path) = environment_path.as_deref() {
        validate_model_cache_path(path)?;
    }
    let environment_limit = std::env::var_os("PANGOPUP_MODEL_CACHE_MAX_ENTRIES")
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| {
                    Failure::usage("PANGOPUP_MODEL_CACHE_MAX_ENTRIES must be valid UTF-8")
                })
                .and_then(|value| {
                    EntryLimit::from_str(value).map_err(|error| Failure::usage(error.to_string()))
                })
        })
        .transpose()?;
    let explicit = cli_path.is_some() || environment_path.is_some();
    let path = if let Some(path) = cli_path.or(environment_path) {
        path
    } else {
        let root = if let Some(root) = std::env::var_os("XDG_CACHE_HOME") {
            let root = PathBuf::from(root);
            if !root.is_absolute() {
                return Err(Failure::usage("XDG_CACHE_HOME must be absolute"));
            }
            root
        } else {
            let home = std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| Failure::usage("HOME is required for the default model cache"))?;
            let home = PathBuf::from(home);
            if !home.is_absolute() {
                return Err(Failure::usage("HOME must be absolute"));
            }
            home.join(".cache")
        };
        root.join("pangopup/model-results.sqlite3")
    };
    Ok(CacheOptions {
        path,
        limit: cli_limit.or(environment_limit).unwrap_or_default(),
        disposable_default: !explicit,
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

fn map_runtime_error(error: AssetError) -> Failure {
    let code = match error.kind() {
        AssetErrorKind::TransportIncompatible => "PROFILE_INCOMPATIBLE",
        AssetErrorKind::StagingInvalid => "PROFILE_UNSAFE",
        AssetErrorKind::BundleInvalid => "PROFILE_CORRUPT",
        _ => error.kind().code(),
    };
    Failure {
        code,
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

fn parse_variant(value: &str) -> Result<Grch38Variant, Failure> {
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
    let contig = parse_contig(contig).ok_or_else(|| Failure::variant("invalid contig spelling"))?;
    let position = position
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| position.parse::<u32>().ok())
        .flatten()
        .filter(|value| *value != 0)
        .ok_or_else(|| Failure::variant("position must be a nonzero decimal u32"))?;
    Grch38Variant::new(
        contig,
        GenomicPosition::new(position).map_err(|error| Failure::variant(error.to_string()))?,
        reference,
        alternate,
    )
    .map_err(|error| Failure::variant(error.to_string()))
}

fn parse_contig(value: &str) -> Option<Grch38Contig> {
    value.parse::<Grch38Contig>().ok().or_else(|| {
        (1_u8..=25).find_map(|code| {
            let contig = Grch38Contig::from_code(code).ok()?;
            (required_accession(contig) == value).then_some(contig)
        })
    })
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
    use std::fs;

    fn repository_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn lookup_arguments(values: &[String]) -> Arguments {
        let raw: Vec<OsString> = values.iter().map(OsString::from).collect();
        let Command::Lookup(arguments) = parse_command(&raw).expect("parse lookup") else {
            panic!("lookup command")
        };
        arguments
    }

    fn fallback_values(cache: &Path) -> Vec<String> {
        vec![
            "--reference-bundle".to_owned(),
            repository_path("tests/fixtures/reference-route-test/bundle")
                .display()
                .to_string(),
            "--mask".to_owned(),
            repository_path("tests/fixtures/route-mask/domains.pgm")
                .display()
                .to_string(),
            "--model-bundle".to_owned(),
            repository_path("tests/fixtures/pangolin-model-kernel-mini/bundle")
                .display()
                .to_string(),
            "--model-cache".to_owned(),
            cache.display().to_string(),
        ]
    }

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

    #[test]
    fn runtime_asset_grammar_is_closed_and_status_json_is_exact() {
        let temp = tempfile::TempDir::new().expect("temp");
        let data = temp.path().join("data");
        let status = [
            OsString::from("assets"),
            OsString::from("runtime"),
            OsString::from("status"),
            OsString::from("--data-dir"),
            data.clone().into_os_string(),
        ];
        let bytes = run(&status).expect("missing runtime status");
        assert_eq!(
            bytes,
            format!(
                "{{\"status\":\"missing\",\"data_dir\":{}}}\n",
                serde_json::to_string(&data).expect("path JSON")
            )
            .as_bytes()
        );

        let complete = [
            "assets",
            "runtime",
            "install",
            "--profile",
            "/tmp/profile.json",
            "--model-bundle",
            "/tmp/model",
            "--reference-bundle",
            "/tmp/reference",
            "--mask",
            "/tmp/domains.pgm",
            "--data-dir",
            "/tmp/pangopup-data",
        ]
        .map(OsString::from);
        assert!(matches!(
            parse_command(&complete).expect("runtime install grammar"),
            Command::RuntimeInstall { .. }
        ));
        for invalid in [
            vec!["assets", "runtime"],
            vec!["assets", "runtime", "start"],
            vec!["assets", "runtime", "status", "--mask", "/tmp/mask"],
            vec!["assets", "runtime", "install", "--profile", "/tmp/profile"],
        ] {
            let raw: Vec<OsString> = invalid.into_iter().map(OsString::from).collect();
            let Err(error) = parse_command(&raw) else {
                panic!("invalid grammar was accepted");
            };
            assert_eq!(error.code, "CLI_USAGE");
        }
    }

    #[test]
    fn general_variant_parser_is_closed_and_resolves_refseq_without_assets() {
        for value in [
            "GRCh38:chr1:5051:A:AC",
            "GRCh38:1:5051:AA:A",
            "GRCh38:NC_000001.11:5051:A:C",
        ] {
            assert!(parse_variant(value).is_ok(), "{value}");
        }
        for value in [
            "GRCh37:chr1:5051:A:C",
            "GRCh38:chr1:0:A:C",
            "GRCh38:chr1:4294967296:A:C",
            "GRCh38:chr1:5051::C",
            "GRCh38:chr1:5051:A:",
            "GRCh38:chr1:5051:a:C",
            "GRCh38:chr1:5051:N:C",
            "GRCh38:chr1:5051:A:A",
            "GRCh38:chrUn:5051:A:C",
            "GRCh38:NC_000001.10:5051:A:C",
            " GRCh38:chr1:5051:A:C",
        ] {
            let failure = parse_variant(value).expect_err(value);
            assert_eq!(failure.code, "INVALID_VARIANT", "{value}");
            assert_eq!(failure.exit, 2, "{value}");
        }
    }

    #[test]
    fn fallback_flags_are_all_or_none_and_duplicates_are_usage_errors() {
        for tail in [
            vec!["--model-bundle", "/m"],
            vec!["--model-bundle", "/m", "--reference-bundle", "/r"],
            vec!["--model-cache", "/tmp/cache.sqlite3"],
            vec!["--model-cache-max-entries", "0"],
            vec![
                "--model-bundle",
                "/m",
                "--reference-bundle",
                "/r",
                "--mask",
                "/x",
                "--mask",
                "/y",
            ],
        ] {
            let mut raw = vec![
                OsString::from("lookup"),
                OsString::from("--bundle"),
                OsString::from("/b"),
                OsString::from("--variant"),
                OsString::from("GRCh38:chr1:1:A:C"),
            ];
            raw.extend(tail.into_iter().map(OsString::from));
            let failure = match parse_command(&raw) {
                Ok(_) => panic!("invalid fallback grammar must fail"),
                Err(failure) => failure,
            };
            assert_eq!(failure.code, "CLI_USAGE");
            assert_eq!(failure.exit, 2);
        }
    }

    #[test]
    fn authoritative_hit_ignores_nonexistent_fallback_paths() {
        let mut values = vec![
            "lookup".to_owned(),
            "--bundle".to_owned(),
            repository_path("tests/fixtures/snv-regression/bundle")
                .display()
                .to_string(),
            "--variant".to_owned(),
            "GRCh38:chr12:6801301:G:A".to_owned(),
            "--model-bundle".to_owned(),
            "/does/not/exist/model".to_owned(),
            "--reference-bundle".to_owned(),
            "/does/not/exist/reference".to_owned(),
            "--mask".to_owned(),
            "/does/not/exist/mask".to_owned(),
        ];
        let arguments = lookup_arguments(&values);
        let mut opens = Vec::new();
        let output = run_lookup_with_observer(arguments, &mut |component| opens.push(component))
            .expect("authoritative hit");
        assert!(opens.is_empty(), "hit-only route must not inspect fallback");
        assert!(
            String::from_utf8(output)
                .expect("UTF-8 output")
                .contains("\"kind\":\"precomputed\"")
        );
        values.clear();
    }

    #[test]
    fn mixed_batch_opens_each_fallback_component_once_and_preserves_order() {
        let temp = tempfile::tempdir().expect("temp");
        fs::set_permissions(
            temp.path(),
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .expect("private temp");
        let mut values = vec![
            "lookup".to_owned(),
            "--bundle".to_owned(),
            repository_path("tests/fixtures/snv-regression/bundle")
                .display()
                .to_string(),
            "--variant".to_owned(),
            "GRCh38:chr12:6801301:G:A".to_owned(),
            "--variant".to_owned(),
            "GRCh38:chr1:5051:A:AC".to_owned(),
        ];
        values.extend(fallback_values(&temp.path().join("cache.sqlite3")));
        let arguments = lookup_arguments(&values);
        let mut opens = Vec::new();
        let output = run_lookup_with_observer(arguments, &mut |component| opens.push(component))
            .expect("mixed batch");
        assert_eq!(
            opens,
            vec![
                FallbackComponent::Reference,
                FallbackComponent::Mask,
                FallbackComponent::Model
            ]
        );
        let text = std::str::from_utf8(&output).expect("UTF-8");
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"kind\":\"precomputed\""));
        assert!(lines[1].contains("\"kind\":\"model\""));

        let mut reopened = Vec::new();
        let reopened_output =
            run_lookup_with_observer(lookup_arguments(&values), &mut |component| {
                reopened.push(component)
            })
            .expect("reopened cache hit");
        assert_eq!(reopened_output, output);
        assert!(
            reopened.is_empty(),
            "a reopened cache hit must not construct fallback components"
        );

        let mut filtered_values = vec![
            "lookup".to_owned(),
            "--bundle".to_owned(),
            repository_path("tests/fixtures/snv-regression/bundle")
                .display()
                .to_string(),
            "--variant".to_owned(),
            "GRCh38:chr1:5051:A:AC".to_owned(),
            "--gene".to_owned(),
            "ENSG00000000001".to_owned(),
        ];
        filtered_values.extend(fallback_values(&temp.path().join("cache.sqlite3")));
        let mut filtered_opens = Vec::new();
        let filtered =
            run_lookup_with_observer(lookup_arguments(&filtered_values), &mut |component| {
                filtered_opens.push(component)
            })
            .expect("filtered cache hit");
        assert!(filtered_opens.is_empty());
        assert!(
            String::from_utf8(filtered)
                .expect("UTF-8")
                .contains("ENSG00000000001.1")
        );
    }

    #[test]
    fn fallback_component_failures_are_ordered_stable_and_redacted() {
        let good_reference = repository_path("tests/fixtures/reference-route-test/bundle");
        let good_mask = repository_path("tests/fixtures/route-mask/domains.pgm");
        let good_model = repository_path("tests/fixtures/pangolin-model-kernel-mini/bundle");
        let secret = repository_path("target/secret-component-path");
        if secret.exists() {
            fs::remove_dir_all(&secret).expect("remove stale secret path");
        }
        for (paths, expected_code, expected_opens) in [
            (
                FallbackPaths {
                    reference: secret.clone(),
                    mask: secret.clone(),
                    model: secret.clone(),
                },
                "REFERENCE_BUNDLE_INVALID",
                vec![FallbackComponent::Reference],
            ),
            (
                FallbackPaths {
                    reference: good_reference.clone(),
                    mask: secret.clone(),
                    model: secret.clone(),
                },
                "MASK_INVALID",
                vec![FallbackComponent::Reference, FallbackComponent::Mask],
            ),
            (
                FallbackPaths {
                    reference: good_reference.clone(),
                    mask: good_mask.clone(),
                    model: secret.clone(),
                },
                "MODEL_BUNDLE_INVALID",
                vec![
                    FallbackComponent::Reference,
                    FallbackComponent::Mask,
                    FallbackComponent::Model,
                ],
            ),
        ] {
            let mut opens = Vec::new();
            let failure = match open_model_fallback(&paths, &mut |component| opens.push(component))
            {
                Ok(_) => panic!("component open must fail"),
                Err(failure) => failure,
            };
            assert_eq!(failure.code, expected_code);
            assert_eq!(failure.exit, 1);
            assert_eq!(opens, expected_opens);
            assert!(!failure.message.contains("secret-component-path"));
        }
        let paths = FallbackPaths {
            reference: good_reference,
            mask: good_mask,
            model: good_model,
        };
        let mut opens = Vec::new();
        open_model_fallback(&paths, &mut |component| opens.push(component))
            .expect("complete fallback");
        assert_eq!(
            opens,
            vec![
                FallbackComponent::Reference,
                FallbackComponent::Mask,
                FallbackComponent::Model
            ]
        );
    }

    #[test]
    fn model_required_missing_assets_and_rejection_have_stable_classes() {
        let temp = tempfile::tempdir().expect("temp");
        fs::set_permissions(
            temp.path(),
            <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .expect("private temp");
        let base = vec![
            "lookup".to_owned(),
            "--bundle".to_owned(),
            repository_path("tests/fixtures/snv-regression/bundle")
                .display()
                .to_string(),
            "--variant".to_owned(),
            "GRCh38:chr1:5051:A:AC".to_owned(),
        ];
        let failure = run_lookup(lookup_arguments(&base)).expect_err("assets required");
        assert_eq!(failure.code, "MODEL_ASSETS_REQUIRED");
        assert_eq!(failure.exit, 2);

        let mut rejected = base;
        rejected[4] = "GRCh38:chr1:5051:A:TC".to_owned();
        rejected.extend(fallback_values(&temp.path().join("cache.sqlite3")));
        let failure = run_lookup(lookup_arguments(&rejected)).expect_err("model rejection");
        assert_eq!(failure.code, "MODEL_REJECTED");
        assert_eq!(failure.exit, 2);

        let scoring = map_model_fallback_error(ModelFallbackError::Scoring(
            pangopup_core::ModelScoringError::InvalidModelOutput,
        ));
        assert_eq!(scoring.code, "MODEL_SCORING");
        assert_eq!(scoring.exit, 1);
    }
}
