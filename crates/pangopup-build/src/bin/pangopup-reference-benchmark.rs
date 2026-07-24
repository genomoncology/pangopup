//! Identity-bound, preserved-build reference qualification harness.
//!
//! This maintenance binary never builds reference data. It authenticates the
//! immutable Ticket 011 v1 build through held descriptors, measures that exact
//! member, re-authenticates every input, and atomically publishes only the
//! small v2 qualification receipt.

use pangopup_build::reference::{
    ReferenceQualificationMeasurements, evaluate_reference_qualification,
};
use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceProvider};
use pangopup_index::reference::ReferenceBundleOpen;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs::{self, File, Permissions},
    hint::black_box,
    io::{Seek, SeekFrom, Write},
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const PRIOR_CONTRACT: &str =
    "sha256:1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01";
const PRODUCTION_PROFILE: &str = "refseq-grch38p14-primary-v1";
const CASES_BYTES: u64 = 220_071;
const CASES_SHA256: &str =
    "sha256:2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const SOURCE: IdentityRef = IdentityRef {
    bytes: 972_898_531,
    sha256: "sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3",
};
const ASSEMBLY_REPORT: IdentityRef = IdentityRef {
    bytes: 80_454,
    sha256: "sha256:64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38",
};
const PRIOR_BUILDER_SOURCE: &str =
    "sha256:2215dabe7c5e81bde9254a7aa8979c78322647d6a5de803976266d9422ea1d8f";
const EXPECTED_FAILURE: &[u8] = b"reference benchmark failed: corpus case\n";
const HARNESS_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/bin/pangopup-reference-benchmark.rs"
));
const SOURCE_INVENTORY: &str = concat!("sha256:", env!("PANGOPUP_BUILDER_SOURCE_SHA256"));
const MAX_SMALL: u64 = 1_048_576;
const MAX_EXECUTABLE: u64 = 64 * 1024 * 1024;
const MAX_REFERENCE: u64 = 773_124_288;

struct CountingAllocator;
static TRACK_COPY: AtomicBool = AtomicBool::new(false);
static TRACK_OPEN: AtomicBool = AtomicBool::new(false);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static CURRENT: AtomicU64 = AtomicU64::new(0);
static OPEN_PEAK: AtomicU64 = AtomicU64::new(0);

fn add(size: usize) {
    let current = CURRENT.fetch_add(size as u64, Ordering::SeqCst) + size as u64;
    if TRACK_COPY.load(Ordering::Relaxed) {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size as u64, Ordering::Relaxed);
    }
    if TRACK_OPEN.load(Ordering::Relaxed) {
        OPEN_PEAK.fetch_max(current, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegate the unchanged allocation to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: pointer/layout are the original allocator pair.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: delegate the unchanged old pair and requested new size.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                add(new_size - layout.size());
            } else {
                CURRENT.fetch_sub((layout.size() - new_size) as u64, Ordering::SeqCst);
            }
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Copy)]
struct IdentityRef {
    bytes: u64,
    sha256: &'static str,
}

impl IdentityRef {
    fn matches(self, identity: &Identity) -> bool {
        self.bytes == identity.bytes && self.sha256 == identity.sha256
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Method {
    rounds: u64,
    warmups_per_round: u64,
    operations_per_round: u64,
    quantile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReuseInput {
    schema: String,
    profile: String,
    source: Identity,
    assembly_report: Identity,
    prior_build: PriorBuildInput,
    prior_failure: PriorFailureInput,
    replacement: ReplacementInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PriorBuildInput {
    contract_id: String,
    qualification_input: Identity,
    build_command: Identity,
    builder_executable: Identity,
    builder_source_sha256: String,
    builder_stdout: Identity,
    builder_stderr: Identity,
    builder_heap_report: Identity,
    builder_resource_log: Identity,
    bundle_id: String,
    manifest: Identity,
    notice: Identity,
    reference_member: Identity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PriorFailureInput {
    benchmark_executable: Identity,
    harness_source_sha256: String,
    qualification_command: Identity,
    qualification_report: Identity,
    qualification_stderr: Identity,
    qualification_resource_log: Identity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplacementInput {
    benchmark_executable: Identity,
    harness_source_sha256: String,
    source_inventory_sha256: String,
    corpus: Identity,
    rust_version: String,
    workload: Method,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V1Input {
    schema: String,
    profile: String,
    builder_executable: Identity,
    benchmark_executable: Identity,
    builder_source_sha256: String,
    harness_source_sha256: String,
    source: Identity,
    assembly_report: Identity,
    corpus: Identity,
    rust_version: String,
    workload: Method,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuilderHeapReport {
    schema: String,
    allocator: String,
    peak_outstanding_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildOutput {
    ok: bool,
    command: String,
    profile: String,
    bundle_id: String,
    members: Vec<BuildMember>,
    certification: BuildCertification,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildMember {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildCertification {
    total_bases: u64,
    sequence_set_sha256: String,
    contexts_verified: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CommandReceipt {
    schema: String,
    working_directory: String,
    argv: [String; 9],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ResourceOutcome {
    QualificationPassedBeforePublication,
    PreflightFailed,
    MeasurementFailed,
    PublicationFailed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ResourceReceipt {
    schema: String,
    started_unix_seconds: u64,
    elapsed_ns: u64,
    maximum_rss_bytes: u64,
    minor_faults: u64,
    major_faults: u64,
    user_cpu_ns: u64,
    system_cpu_ns: u64,
    outcome: ResourceOutcome,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    qualification_contract_id: String,
    prior_build_contract_id: String,
    passed: bool,
    identities: ReportIdentities,
    logical: Logical,
    method: Method,
    performance: Performance,
    storage: Storage,
    resources: Resources,
    host: Host,
    mmap_rss_interpretation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportIdentities {
    source: Identity,
    assembly_report: Identity,
    prior_build: PriorBuildReportIdentities,
    prior_failure: PriorFailureReportIdentities,
    replacement: ReplacementReportIdentities,
    retained: RetainedIdentities,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PriorBuildReportIdentities {
    qualification_input: Identity,
    build_command: Identity,
    builder_executable: Identity,
    builder_source_sha256: String,
    builder_stdout: Identity,
    builder_stderr: Identity,
    builder_heap_report: Identity,
    builder_resource_log: Identity,
    bundle_id: String,
    manifest: Identity,
    notice: Identity,
    reference_member: Identity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PriorFailureReportIdentities {
    benchmark_executable: Identity,
    harness_source_sha256: String,
    qualification_command: Identity,
    qualification_report: Identity,
    qualification_stderr: Identity,
    qualification_resource_log: Identity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplacementReportIdentities {
    benchmark_executable: Identity,
    harness_source_sha256: String,
    source_inventory_sha256: String,
    corpus: Identity,
    rust_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RetainedIdentities {
    reuse_input: Identity,
    command: Identity,
    resource: Identity,
    stderr: Identity,
    benchmark_executable: Identity,
    harness_source: Identity,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Logical {
    total_bases: u64,
    sequence_set_sha256: String,
    extra_record_count: u64,
    extra_accessions_sha256: String,
    contexts_verified: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Performance {
    open_ns: Vec<u64>,
    round_p50_ns: Vec<u64>,
    round_p95_ns: Vec<u64>,
    headline_p50_ns: u64,
    headline_p95_ns: u64,
    allocation_calls: u64,
    allocation_bytes: u64,
    unique_dense_pages: Vec<u64>,
    per_case_page_count_sum: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Storage {
    installed_bundle_bytes: u64,
    reference_member_bytes: u64,
    pinned_zstandard_bytes: u64,
    pinned_zstandard: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    builder_peak_heap_bytes: u64,
    open_peak_heap_bytes: u64,
    dense_bytes_read_during_open: u64,
    builder_maximum_rss_bytes: u64,
    builder_minor_faults: u64,
    builder_major_faults: u64,
    benchmark_maximum_rss_bytes: u64,
    benchmark_minor_faults: u64,
    benchmark_major_faults: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Host {
    hostname: String,
    kernel: String,
    cpu: String,
    os: String,
    architecture: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum FailurePhase {
    Preflight,
    Measurement,
    Publication,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReportState {
    Empty,
    CompleteUnpublished,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FailureReceipt {
    schema: String,
    qualification_contract_id: String,
    prior_build_contract_id: String,
    phase: FailurePhase,
    code: String,
    message: String,
    report_state: ReportState,
    evidence: FailureEvidence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FailureEvidence {
    reuse_input: Identity,
    command: Identity,
    resource: Identity,
    stderr: Identity,
    report: Identity,
    benchmark_executable: Identity,
    harness_source: Identity,
}

#[derive(Clone, Copy, Debug)]
struct Usage {
    maximum_rss_bytes: u64,
    minor_faults: u64,
    major_faults: u64,
    user_cpu_ns: u64,
    system_cpu_ns: u64,
}

#[derive(Clone, Copy, Debug)]
struct HeldStat {
    device: u64,
    inode: u64,
    size: u64,
}

struct HeldFile {
    file: File,
    stat: HeldStat,
    identity: Identity,
    authenticated_bytes: Option<Vec<u8>>,
    descriptor_read_passes: u64,
}

struct HeldEvidence {
    files: BTreeMap<&'static str, HeldFile>,
}

struct Arguments {
    reuse_input: PathBuf,
    prior_root: PathBuf,
    corpus: PathBuf,
    output: PathBuf,
    raw_argv: [String; 9],
}

#[derive(Debug)]
struct Failure {
    phase: FailurePhase,
    code: &'static str,
    message: String,
}

struct FailureTiming {
    started: u64,
    started_at: Instant,
    usage_before: Usage,
}

struct FailureIdentities<'a> {
    reuse: &'a Identity,
    command: &'a Identity,
    executable: &'a Identity,
    harness: &'a Identity,
}

impl Failure {
    fn preflight(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            phase: FailurePhase::Preflight,
            code,
            message: message.into(),
        }
    }
    fn measurement(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            phase: FailurePhase::Measurement,
            code,
            message: message.into(),
        }
    }
    fn publication(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            phase: FailurePhase::Publication,
            code,
            message: message.into(),
        }
    }
}

#[derive(Deserialize)]
struct CaseEnvelope {
    id: String,
    input: CaseInput,
    context: CaseContext,
}
#[derive(Deserialize)]
struct CaseInput {
    contig: String,
}
#[derive(Deserialize)]
struct CaseContext {
    start_1based: u32,
    bases: String,
}
#[derive(Debug)]
struct Case {
    id: String,
    contig: Grch38Contig,
    start: GenomicPosition,
    bases: Vec<u8>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!(
            "reference qualification failed: {}: {}",
            error.code, error.message
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), Failure> {
    #[cfg(not(feature = "reference-qualification"))]
    return Err(Failure::preflight(
        "FEATURE_DISABLED",
        "qualification feature is disabled",
    ));

    #[cfg(feature = "reference-qualification")]
    run_qualification()
}

#[cfg(feature = "reference-qualification")]
fn run_qualification() -> Result<(), Failure> {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::preflight("CLOCK", "system clock is unavailable"))?
        .as_secs();
    let started_at = Instant::now();
    let usage_before =
        usage().map_err(|_| Failure::preflight("RESOURCE", "resource counters unavailable"))?;
    let arguments = parse_arguments()?;

    let mut reuse_file = open_and_read_absolute(&arguments.reuse_input, 65_536)?;
    let retained_reuse_bytes = authenticated_bytes(&reuse_file, 65_536)?.to_vec();
    let reuse_identity = reuse_file.identity.clone();
    let contract_id = reuse_identity.sha256.clone();
    validate_sha(&contract_id)?;
    let contract_leaf = contract_id
        .strip_prefix("sha256:")
        .ok_or_else(|| Failure::preflight("INPUT_IDENTITY", "invalid input digest"))?;
    let output_leaf = arguments
        .output
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| Failure::preflight("OUTPUT", "output name is invalid"))?;
    if output_leaf != contract_leaf {
        return Err(Failure::preflight(
            "OUTPUT",
            "output name does not match contract",
        ));
    }
    let output_parent_path = arguments
        .output
        .parent()
        .ok_or_else(|| Failure::preflight("OUTPUT", "output parent is missing"))?;
    let output_parent = open_absolute_directory(output_parent_path)?;
    ensure_child_absent(&output_parent, OsStr::new(output_leaf))?;
    let command = CommandReceipt {
        schema: "pangopup-reference-qualification-command-v2".to_owned(),
        working_directory: std::env::current_dir()
            .ok()
            .and_then(|path| path.into_os_string().into_string().ok())
            .ok_or_else(|| {
                Failure::preflight("WORKING_DIRECTORY", "working directory is unavailable")
            })?,
        argv: arguments.raw_argv.clone(),
    };
    let command_bytes = canonical(&command, "command")?;
    let command_identity = identity_bytes(&command_bytes);
    let harness_identity = identity_bytes(HARNESS_SOURCE);
    let mut executable = open_current_executable()?;
    let executable_identity = executable.identity.clone();
    let mut stage = QualificationStage::create(&output_parent, output_parent_path, contract_leaf)?;

    let result = run_staged(
        &mut stage,
        &output_parent,
        output_leaf,
        &arguments,
        &mut reuse_file,
        &reuse_identity,
        &command_bytes,
        &command_identity,
        &mut executable,
        &executable_identity,
        &harness_identity,
        &contract_id,
        started,
        started_at,
        usage_before,
    );
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let phase = error.phase;
            let code = error.code;
            let message = error.message.clone();
            seal_and_publish_failure(
                &mut stage,
                &output_parent,
                contract_leaf,
                PRIOR_CONTRACT,
                error,
                FailureTiming {
                    started,
                    started_at,
                    usage_before,
                },
                FailureIdentities {
                    reuse: &reuse_identity,
                    command: &command_identity,
                    executable: &executable_identity,
                    harness: &harness_identity,
                },
                &retained_reuse_bytes,
                &command_bytes,
                &executable,
            )?;
            Err(Failure {
                phase,
                code,
                message,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_staged(
    stage: &mut QualificationStage,
    output_parent: &File,
    output_leaf: &str,
    arguments: &Arguments,
    reuse_file: &mut HeldFile,
    reuse_identity: &Identity,
    command_bytes: &[u8],
    command_identity: &Identity,
    executable: &mut HeldFile,
    executable_identity: &Identity,
    harness_identity: &Identity,
    contract_id: &str,
    started: u64,
    started_at: Instant,
    usage_before: Usage,
) -> Result<(), Failure> {
    let reuse_bytes = authenticated_bytes(reuse_file, 65_536)?;

    // Retain the immutable inputs before any interpretation. If later
    // preflight fails, the same stage becomes the durable failure receipt.
    stage.write_file("reuse-input.json", reuse_bytes, 0o600)?;
    stage.write_file("command.json", command_bytes, 0o600)?;
    stage.mkdir("candidate", 0o700)?;
    stage.copy_held("candidate/pangopup-reference-benchmark", executable, 0o500)?;
    stage.write_file(
        "candidate/pangopup-reference-benchmark.rs",
        HARNESS_SOURCE,
        0o600,
    )?;
    stage.write_file("qualification-report.json", &[], 0o600)?;
    stage.write_file("qualification-resource.json", &[], 0o600)?;
    stage.write_file("qualification.stderr.log", &[], 0o600)?;

    if stage_member_identity(
        stage,
        "candidate/pangopup-reference-benchmark",
        MAX_EXECUTABLE,
    )? != *executable_identity
        || stage_member_identity(
            stage,
            "candidate/pangopup-reference-benchmark.rs",
            MAX_SMALL,
        )? != *harness_identity
    {
        return Err(Failure::preflight(
            "STAGE_IDENTITY",
            "retained candidate copy is inconsistent",
        ));
    }
    let input: ReuseInput = parse_canonical(reuse_bytes, "reuse input")?;
    validate_reuse_input(
        &input,
        reuse_identity,
        executable_identity,
        harness_identity,
    )?;
    let corpus_file = open_and_read_absolute(&arguments.corpus, MAX_SMALL)?;
    require_identity(&corpus_file.identity, &input.replacement.corpus, "corpus")?;
    let cases = parse_cases(authenticated_bytes(&corpus_file, MAX_SMALL)?)?;
    let prior_root = open_absolute_directory(&arguments.prior_root)?;
    let evidence = open_prior_evidence(&prior_root, &input)?;
    validate_prior_evidence(&input, &evidence)?;
    let mut preflight = Preflight {
        input,
        evidence,
        corpus: corpus_file,
        cases,
    };
    let measurement = measure(&mut preflight)?;
    revalidate_all(&mut preflight, reuse_file, executable)?;
    let usage_after =
        usage().map_err(|_| Failure::measurement("RESOURCE", "resource counters unavailable"))?;
    let success_resource = resource_receipt(
        started,
        started_at,
        usage_before,
        usage_after,
        ResourceOutcome::QualificationPassedBeforePublication,
    )?;
    let resource_bytes = canonical(&success_resource, "resource")?;
    let resource_identity = identity_bytes(&resource_bytes);
    let empty_identity = identity_bytes(&[]);
    let report = build_report(
        contract_id,
        &preflight,
        measurement,
        RetainedIdentities {
            reuse_input: reuse_identity.clone(),
            command: command_identity.clone(),
            resource: resource_identity.clone(),
            stderr: empty_identity.clone(),
            benchmark_executable: executable_identity.clone(),
            harness_source: harness_identity.clone(),
        },
    );
    let report_bytes = canonical(&report, "report")?;
    // The closed report must parse and round-trip before it can become evidence.
    let _: Report = parse_canonical(&report_bytes, "report")?;
    stage.replace_file("qualification-resource.json", &resource_bytes, 0o600)?;
    stage.replace_file("qualification-report.json", &report_bytes, 0o600)?;
    stage.seal_success().and_then(|()| {
        validate_and_publish_success(stage, output_parent, OsStr::new(output_leaf), &report)
    })
}

struct Preflight {
    input: ReuseInput,
    evidence: HeldEvidence,
    corpus: HeldFile,
    cases: Vec<Case>,
}

struct Measurement {
    logical: Logical,
    method: Method,
    performance: Performance,
    storage: Storage,
    resources: Resources,
    host: Host,
}

fn parse_arguments() -> Result<Arguments, Failure> {
    let values: Vec<OsString> = std::env::args_os().collect();
    let [
        program,
        reuse_flag,
        reuse,
        prior_flag,
        prior,
        corpus_flag,
        corpus,
        output_flag,
        output,
    ] = values.as_slice()
    else {
        return Err(Failure::preflight(
            "CLI_USAGE",
            "qualification arguments are invalid",
        ));
    };
    if reuse_flag != "--reuse-input"
        || prior_flag != "--prior-root"
        || corpus_flag != "--corpus"
        || output_flag != "--output"
    {
        return Err(Failure::preflight(
            "CLI_USAGE",
            "qualification arguments are invalid",
        ));
    }
    let strings: Vec<String> = values
        .iter()
        .map(|value| value.clone().into_string())
        .collect::<Result<_, _>>()
        .map_err(|_| Failure::preflight("CLI_USAGE", "qualification arguments are invalid"))?;
    let raw_argv: [String; 9] = strings
        .try_into()
        .map_err(|_| Failure::preflight("CLI_USAGE", "qualification arguments are invalid"))?;
    let _ = program;
    Ok(Arguments {
        reuse_input: reuse.into(),
        prior_root: prior.into(),
        corpus: corpus.into(),
        output: output.into(),
        raw_argv,
    })
}

fn validate_reuse_input(
    input: &ReuseInput,
    input_identity: &Identity,
    executable: &Identity,
    harness: &Identity,
) -> Result<(), Failure> {
    validate_identity(input_identity)?;
    for identity in all_input_identities(input) {
        validate_identity(identity)?;
    }
    for digest in [
        &input.prior_build.contract_id,
        &input.prior_build.builder_source_sha256,
        &input.prior_build.bundle_id,
        &input.prior_failure.harness_source_sha256,
        &input.replacement.harness_source_sha256,
        &input.replacement.source_inventory_sha256,
    ] {
        validate_sha(digest)?;
    }
    if input.schema != "pangopup-reference-qualification-reuse-input-v2"
        || input.profile != PRODUCTION_PROFILE
        || input.prior_build.contract_id != PRIOR_CONTRACT
        || !SOURCE.matches(&input.source)
        || !ASSEMBLY_REPORT.matches(&input.assembly_report)
        || input.prior_build.builder_source_sha256 != PRIOR_BUILDER_SOURCE
        || input.replacement.corpus.bytes != CASES_BYTES
        || input.replacement.corpus.sha256 != CASES_SHA256
        || input.replacement.rust_version != env!("PANGOPUP_RUSTC_VERSION")
        || input.replacement.source_inventory_sha256 != SOURCE_INVENTORY
        || &input.replacement.benchmark_executable != executable
        || input.replacement.harness_source_sha256 != harness.sha256
        || input.replacement.workload
            != (Method {
                rounds: 5,
                warmups_per_round: 20,
                operations_per_round: 10_000,
                quantile: "nearest-rank".to_owned(),
            })
    {
        return Err(Failure::preflight(
            "INPUT_CONTRACT",
            "reuse input does not match the accepted contract",
        ));
    }
    validate_known_prior_identities(input)?;
    Ok(())
}

fn validate_known_prior_identities(input: &ReuseInput) -> Result<(), Failure> {
    let expected = [
        (
            &input.prior_build.qualification_input,
            1029,
            "sha256:1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01",
        ),
        (
            &input.prior_build.build_command,
            1409,
            "sha256:6c43e18dc6c4ca9839a3a72c5ed2f0d365cadc6a53e8442b9ef2027add54600e",
        ),
        (
            &input.prior_build.builder_executable,
            5_167_976,
            "sha256:5d503b9dc8e9968e83d7657ac9cf617474c854d13a8f756803757aaab33ed7dc",
        ),
        (
            &input.prior_build.builder_stdout,
            577,
            "sha256:d3a3aa536452484b911f0f64fdf4794c1595963e52810deabbd308604cbd45c2",
        ),
        (
            &input.prior_build.builder_stderr,
            0,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            &input.prior_build.builder_heap_report,
            118,
            "sha256:eb035a24de0f1a8bc23e316c06be3529803a30e8dfac284f21f71ed5cfd00204",
        ),
        (
            &input.prior_build.builder_resource_log,
            1641,
            "sha256:0fbafdceb9541fc866fdb55a51c6b35b14b341df157544d1a2c4da68f53622b3",
        ),
        (
            &input.prior_build.manifest,
            3719,
            "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f",
        ),
        (
            &input.prior_build.notice,
            793,
            "sha256:1e3ce49d78cd9089407c54ce92a9e6d3adb92a9f3267185ba9ea64df8a588499",
        ),
        (
            &input.prior_build.reference_member,
            772_091_760,
            "sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82",
        ),
        (
            &input.prior_failure.benchmark_executable,
            1_777_632,
            "sha256:11adc18be7ee659b08ab79a62d83102ec87585e33d38db75054ac0dc25a71072",
        ),
        (
            &input.prior_failure.qualification_command,
            1598,
            "sha256:4641bb9420eca169d60fd89a6afe3c7f32dedc594a04baf76c549a1d7519ef38",
        ),
        (
            &input.prior_failure.qualification_report,
            0,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            &input.prior_failure.qualification_stderr,
            40,
            "sha256:0a4dbf073617ad47c4a525e31559a8c240296259b70e8b858080562008dc9cb2",
        ),
        (
            &input.prior_failure.qualification_resource_log,
            1823,
            "sha256:c5d8349995dc2099104c8dd597fe0c766e7b1f8e4404be7424d7c5b099d07256",
        ),
    ];
    if input.prior_build.bundle_id
        != "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f"
        || input.prior_failure.harness_source_sha256
            != "sha256:e359876ab8e7a8b06761ec03c3df4f9bbe60f427db7f8c85c0374c007c2b0a21"
        || expected
            .into_iter()
            .any(|(actual, bytes, sha256)| actual.bytes != bytes || actual.sha256 != sha256)
    {
        return Err(Failure::preflight(
            "PRIOR_IDENTITY",
            "retained v1 identity is not the accepted immutable evidence",
        ));
    }
    Ok(())
}

fn all_input_identities(input: &ReuseInput) -> Vec<&Identity> {
    vec![
        &input.source,
        &input.assembly_report,
        &input.prior_build.qualification_input,
        &input.prior_build.build_command,
        &input.prior_build.builder_executable,
        &input.prior_build.builder_stdout,
        &input.prior_build.builder_stderr,
        &input.prior_build.builder_heap_report,
        &input.prior_build.builder_resource_log,
        &input.prior_build.manifest,
        &input.prior_build.notice,
        &input.prior_build.reference_member,
        &input.prior_failure.benchmark_executable,
        &input.prior_failure.qualification_command,
        &input.prior_failure.qualification_report,
        &input.prior_failure.qualification_stderr,
        &input.prior_failure.qualification_resource_log,
        &input.replacement.benchmark_executable,
        &input.replacement.corpus,
    ]
}

fn open_prior_evidence(root: &File, input: &ReuseInput) -> Result<HeldEvidence, Failure> {
    let specifications: [(&str, &str, &Identity, u64); 15] = [
        (
            "qualification_input",
            "qualification-input.json",
            &input.prior_build.qualification_input,
            MAX_SMALL,
        ),
        (
            "build_command",
            "command.txt",
            &input.prior_build.build_command,
            MAX_SMALL,
        ),
        (
            "builder_executable",
            "candidate/pangopup-build",
            &input.prior_build.builder_executable,
            MAX_EXECUTABLE,
        ),
        (
            "builder_stdout",
            "builder.stdout.jsonl",
            &input.prior_build.builder_stdout,
            MAX_SMALL,
        ),
        (
            "builder_stderr",
            "builder.stderr.log",
            &input.prior_build.builder_stderr,
            MAX_SMALL,
        ),
        (
            "builder_heap_report",
            "builder-heap-report.json",
            &input.prior_build.builder_heap_report,
            MAX_SMALL,
        ),
        (
            "builder_resource_log",
            "builder-resource.log",
            &input.prior_build.builder_resource_log,
            MAX_SMALL,
        ),
        (
            "manifest",
            "bundle/manifest.json",
            &input.prior_build.manifest,
            65_536,
        ),
        ("notice", "bundle/NOTICE", &input.prior_build.notice, 16_384),
        (
            "reference_member",
            "bundle/reference.pgr",
            &input.prior_build.reference_member,
            MAX_REFERENCE,
        ),
        (
            "prior_benchmark",
            "candidate/pangopup-reference-benchmark",
            &input.prior_failure.benchmark_executable,
            MAX_EXECUTABLE,
        ),
        (
            "prior_command",
            "qualification-command.txt",
            &input.prior_failure.qualification_command,
            MAX_SMALL,
        ),
        (
            "prior_report",
            "qualification-report.json",
            &input.prior_failure.qualification_report,
            MAX_SMALL,
        ),
        (
            "prior_stderr",
            "qualification.stderr.log",
            &input.prior_failure.qualification_stderr,
            MAX_SMALL,
        ),
        (
            "prior_resource",
            "qualification-resource.log",
            &input.prior_failure.qualification_resource_log,
            MAX_SMALL,
        ),
    ];
    let mut files = BTreeMap::new();
    for (key, relative, expected, maximum) in specifications {
        let mut held = open_relative_regular(root, Path::new(relative), maximum)?;
        if maximum <= MAX_SMALL {
            authenticate_held_buffered(&mut held, maximum)?;
        } else {
            authenticate_held(&mut held)?;
        }
        require_identity(&held.identity, expected, key)?;
        files.insert(key, held);
    }
    let mut prior_harness = open_relative_regular(
        root,
        Path::new("candidate/pangopup-reference-benchmark.rs"),
        MAX_SMALL,
    )?;
    authenticate_held_buffered(&mut prior_harness, MAX_SMALL)?;
    if prior_harness.identity.bytes != 26_804
        || prior_harness.identity.sha256 != input.prior_failure.harness_source_sha256
    {
        return Err(Failure::preflight(
            "IDENTITY_MISMATCH",
            "prior harness identity mismatch",
        ));
    }
    files.insert("prior_harness", prior_harness);
    Ok(HeldEvidence { files })
}

fn validate_prior_evidence(input: &ReuseInput, evidence: &HeldEvidence) -> Result<(), Failure> {
    let v1_bytes = evidence.bytes("qualification_input", 65_536)?;
    let v1: V1Input = parse_canonical(v1_bytes, "prior qualification input")?;
    if evidence.identity("qualification_input")?.sha256 != input.prior_build.contract_id
        || v1.schema != "pangopup-reference-qualification-input-v1"
        || v1.profile != input.profile
        || v1.builder_executable != input.prior_build.builder_executable
        || v1.benchmark_executable != input.prior_failure.benchmark_executable
        || v1.builder_source_sha256 != input.prior_build.builder_source_sha256
        || v1.harness_source_sha256 != input.prior_failure.harness_source_sha256
        || v1.source != input.source
        || v1.assembly_report != input.assembly_report
        || v1.corpus != input.replacement.corpus
        || v1.workload != input.replacement.workload
    {
        return Err(Failure::preflight(
            "PRIOR_INPUT",
            "prior input binding is invalid",
        ));
    }

    let output_bytes = evidence.bytes("builder_stdout", MAX_SMALL)?;
    let output = parse_canonical_line::<BuildOutput>(output_bytes, "builder stdout")?;
    if !output.ok
        || output.command != "reference.build"
        || output.profile != PRODUCTION_PROFILE
        || output.bundle_id != input.prior_build.bundle_id
        || output.certification.total_bases != 3_088_286_401
        || output.certification.sequence_set_sha256
            != "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4"
        || output.certification.contexts_verified != 14
        || output.members.len() != 2
        || !output.members.iter().any(|member| {
            member.path == "NOTICE"
                && member.size == input.prior_build.notice.bytes
                && member.sha256 == input.prior_build.notice.sha256
        })
        || !output.members.iter().any(|member| {
            member.path == "reference.pgr"
                && member.size == input.prior_build.reference_member.bytes
                && member.sha256 == input.prior_build.reference_member.sha256
        })
    {
        return Err(Failure::preflight(
            "BUILD_EVIDENCE",
            "builder output is inconsistent",
        ));
    }
    if !evidence.bytes("builder_stderr", MAX_SMALL)?.is_empty()
        || !evidence.bytes("prior_report", MAX_SMALL)?.is_empty()
        || evidence.bytes("prior_stderr", MAX_SMALL)? != EXPECTED_FAILURE
    {
        return Err(Failure::preflight(
            "PRIOR_FAILURE",
            "prior failure evidence is inconsistent",
        ));
    }
    let heap_bytes = evidence.bytes("builder_heap_report", MAX_SMALL)?;
    let heap: BuilderHeapReport = parse_canonical(heap_bytes, "builder heap")?;
    if heap.schema != "pangopup-reference-builder-heap-v1"
        || heap.allocator != "rust-system-outstanding"
        || heap.peak_outstanding_bytes > 16_777_216
    {
        return Err(Failure::preflight(
            "BUILD_RESOURCE",
            "builder heap evidence is invalid",
        ));
    }
    let build_resource = evidence.bytes("builder_resource_log", MAX_SMALL)?;
    if read_time_report(build_resource, 0).is_err() {
        return Err(Failure::preflight(
            "BUILD_RESOURCE",
            "builder resource evidence is invalid",
        ));
    }
    let prior_resource = evidence.bytes("prior_resource", MAX_SMALL)?;
    if read_time_report(prior_resource, 1).is_err() {
        return Err(Failure::preflight(
            "PRIOR_FAILURE",
            "prior resource evidence is invalid",
        ));
    }
    // These are executable shell transcripts rather than JSON. Their complete
    // identities are the authority; require the fixed command verbs as an
    // additional wrong-file control without treating paths as declarations.
    if !contains_ascii(
        evidence.bytes("build_command", MAX_SMALL)?,
        b" reference build ",
    ) || !contains_ascii(
        evidence.bytes("prior_command", MAX_SMALL)?,
        b"pangopup-reference-benchmark --bundle ",
    ) {
        return Err(Failure::preflight(
            "COMMAND_EVIDENCE",
            "prior command evidence is invalid",
        ));
    }
    let manifest = evidence.bytes("manifest", 65_536)?;
    let notice = evidence.bytes("notice", 16_384)?;
    let reader = ReferenceBundleOpen::open_qualification(
        manifest,
        notice,
        &evidence.file("reference_member")?.file,
    )
    .map_err(|_| Failure::preflight("BUNDLE", "retained bundle is structurally invalid"))?;
    let manifest = reader.manifest();
    if reader.provenance().bundle_id() != input.prior_build.bundle_id
        || manifest.profile != PRODUCTION_PROFILE
        || manifest.builder.source_sha256 != input.prior_build.builder_source_sha256
        || manifest.sequences.total_bases != 3_088_286_401
        || manifest.sequences.extra_record_count != 680
        || manifest.sequences.sequence_set_sha256
            != "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4"
        || manifest.sequences.extra_accessions_sha256
            != "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb"
    {
        return Err(Failure::preflight(
            "BUNDLE",
            "retained bundle manifest is inconsistent",
        ));
    }
    Ok(())
}

impl HeldEvidence {
    fn file(&self, key: &'static str) -> Result<&HeldFile, Failure> {
        self.files
            .get(key)
            .ok_or_else(|| Failure::preflight("EVIDENCE", "required evidence is missing"))
    }

    fn identity(&self, key: &'static str) -> Result<&Identity, Failure> {
        Ok(&self.file(key)?.identity)
    }

    fn bytes(&self, key: &'static str, maximum: u64) -> Result<&[u8], Failure> {
        authenticated_bytes(self.file(key)?, maximum)
    }
}

fn measure(preflight: &mut Preflight) -> Result<Measurement, Failure> {
    let manifest_bytes = preflight.evidence.bytes("manifest", 65_536)?;
    let notice_bytes = preflight.evidence.bytes("notice", 16_384)?;
    let reference = preflight.evidence.file("reference_member")?;
    let heap_bytes = preflight.evidence.bytes("builder_heap_report", MAX_SMALL)?;
    let heap: BuilderHeapReport = parse_canonical(heap_bytes, "builder heap")?;
    let build_resource = read_time_report(
        preflight
            .evidence
            .bytes("builder_resource_log", MAX_SMALL)?,
        0,
    )?;
    let maximum = preflight
        .cases
        .iter()
        .map(|case| case.bases.len())
        .max()
        .ok_or_else(|| Failure::measurement("CORPUS", "corpus has no cases"))?;
    let mut destination = vec![0_u8; maximum];
    let method = preflight.input.replacement.workload.clone();
    let rounds = usize::try_from(method.rounds)
        .map_err(|_| Failure::measurement("WORKLOAD", "round count is invalid"))?;
    let operations = usize::try_from(method.operations_per_round)
        .map_err(|_| Failure::measurement("WORKLOAD", "operation count is invalid"))?;
    let mut retained: Vec<Vec<u64>> = (0..rounds).map(|_| vec![0; operations]).collect();
    let mut open_ns = Vec::with_capacity(rounds);
    let mut open_peak = 0_u64;
    let mut dense_open = 0_u64;
    let mut measured_calls = 0_u64;
    let mut measured_bytes = 0_u64;
    for round_timings in &mut retained {
        let baseline = CURRENT.load(Ordering::SeqCst);
        OPEN_PEAK.store(baseline, Ordering::SeqCst);
        TRACK_OPEN.store(true, Ordering::SeqCst);
        let opened_at = Instant::now();
        let (reader, round_dense_open) = ReferenceBundleOpen::open_qualification_audited(
            manifest_bytes,
            notice_bytes,
            &reference.file,
        )
        .map_err(|_| Failure::measurement("BUNDLE_OPEN", "held bundle is invalid"))?;
        let elapsed = opened_at.elapsed().as_nanos();
        TRACK_OPEN.store(false, Ordering::SeqCst);
        open_ns.push(nanos(elapsed)?);
        open_peak = open_peak.max(OPEN_PEAK.load(Ordering::SeqCst).saturating_sub(baseline));
        dense_open = dense_open.saturating_add(round_dense_open);
        verify_cases(&reader, &preflight.cases, &mut destination)?;
        for operation in 0..method.warmups_per_round {
            copy_case(
                &reader,
                &preflight.cases[operation as usize % preflight.cases.len()],
                &mut destination,
            )?;
        }
        CALLS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
        for (operation, timing) in round_timings.iter_mut().enumerate() {
            let case = &preflight.cases[operation % preflight.cases.len()];
            let output = &mut destination[..case.bases.len()];
            TRACK_COPY.store(true, Ordering::SeqCst);
            let started = Instant::now();
            let result = reader.copy_window(case.contig, case.start, output);
            let elapsed = started.elapsed().as_nanos();
            TRACK_COPY.store(false, Ordering::SeqCst);
            result.map_err(|_| Failure::measurement("WINDOW", "window copy failed"))?;
            *timing = nanos(elapsed)?;
            black_box(Sha256::digest(output));
        }
        measured_calls = measured_calls.saturating_add(CALLS.load(Ordering::Relaxed));
        measured_bytes = measured_bytes.saturating_add(BYTES.load(Ordering::Relaxed));
    }
    let reader =
        ReferenceBundleOpen::open_qualification(manifest_bytes, notice_bytes, &reference.file)
            .map_err(|_| Failure::measurement("BUNDLE_OPEN", "held bundle is invalid"))?;
    let mut unique = BTreeSet::new();
    let mut page_sum = 0_u64;
    for case in &preflight.cases {
        let pages = reader
            .trace_window_pages(case.contig, case.start, case.bases.len())
            .map_err(|_| Failure::measurement("PAGE_TRACE", "page trace failed"))?;
        page_sum = page_sum.saturating_add(pages.len() as u64);
        unique.extend(pages);
    }
    let unique: Vec<_> = unique.into_iter().collect();
    let mut round_p50 = Vec::with_capacity(rounds);
    let mut round_p95 = Vec::with_capacity(rounds);
    for values in &mut retained {
        values.sort_unstable();
        round_p50.push(values[4_999]);
        round_p95.push(values[9_499]);
    }
    let mut p50 = round_p50.clone();
    p50.sort_unstable();
    let mut p95 = round_p95.clone();
    p95.sort_unstable();
    let manifest = reader.manifest();
    if manifest.builder.source_sha256 != preflight.input.prior_build.builder_source_sha256
        || manifest.source.fasta.bytes != preflight.input.source.bytes
        || manifest.source.fasta.sha256 != preflight.input.source.sha256
        || manifest.source.assembly_report.bytes != preflight.input.assembly_report.bytes
        || manifest.source.assembly_report.sha256 != preflight.input.assembly_report.sha256
        || reader.provenance().bundle_id() != preflight.input.prior_build.bundle_id
    {
        return Err(Failure::measurement(
            "BUNDLE_IDENTITY",
            "bundle provenance is inconsistent",
        ));
    }
    let reference_identity = preflight.evidence.identity("reference_member")?.clone();
    let installed = preflight
        .evidence
        .identity("manifest")?
        .bytes
        .checked_add(preflight.evidence.identity("notice")?.bytes)
        .and_then(|value| value.checked_add(reference_identity.bytes))
        .ok_or_else(|| Failure::measurement("STORAGE", "installed size overflow"))?;
    let zstd_bytes = zstd_size(&reference.file, reference_identity.bytes)?;
    let measurements = ReferenceQualificationMeasurements {
        total_bases: manifest.sequences.total_bases,
        sequence_set_sha256: &manifest.sequences.sequence_set_sha256,
        extra_record_count: manifest.sequences.extra_record_count,
        extra_accessions_sha256: &manifest.sequences.extra_accessions_sha256,
        contexts_verified: 14,
        headline_p50_ns: p50[2],
        headline_p95_ns: p95[2],
        allocation_calls: measured_calls,
        allocation_bytes: measured_bytes,
        open_peak_heap_bytes: open_peak,
        builder_peak_heap_bytes: heap.peak_outstanding_bytes,
        dense_bytes_read_during_open: dense_open,
        member_bytes: reference_identity.bytes,
        unique_dense_pages: &unique,
        per_case_page_count_sum: page_sum,
    };
    evaluate_reference_qualification(&measurements)
        .map_err(|rejection| Failure::measurement("THRESHOLD", rejection.message))?;
    let current_usage =
        usage().map_err(|_| Failure::measurement("RESOURCE", "resource counters unavailable"))?;
    Ok(Measurement {
        logical: Logical {
            total_bases: measurements.total_bases,
            sequence_set_sha256: measurements.sequence_set_sha256.to_owned(),
            extra_record_count: measurements.extra_record_count,
            extra_accessions_sha256: measurements.extra_accessions_sha256.to_owned(),
            contexts_verified: measurements.contexts_verified,
        },
        method,
        performance: Performance {
            open_ns,
            round_p50_ns: round_p50,
            round_p95_ns: round_p95,
            headline_p50_ns: measurements.headline_p50_ns,
            headline_p95_ns: measurements.headline_p95_ns,
            allocation_calls: measured_calls,
            allocation_bytes: measured_bytes,
            unique_dense_pages: unique,
            per_case_page_count_sum: page_sum,
        },
        storage: Storage {
            installed_bundle_bytes: installed,
            reference_member_bytes: reference_identity.bytes,
            pinned_zstandard_bytes: zstd_bytes,
            pinned_zstandard: "zstd-0.13.3/libzstd-1.5.7;level=9;checksum;content-size;no-dict-id;no-long-distance;workers=0".to_owned(),
        },
        resources: Resources {
            builder_peak_heap_bytes: heap.peak_outstanding_bytes,
            open_peak_heap_bytes: open_peak,
            dense_bytes_read_during_open: dense_open,
            builder_maximum_rss_bytes: build_resource.maximum_rss_bytes,
            builder_minor_faults: build_resource.minor_faults,
            builder_major_faults: build_resource.major_faults,
            benchmark_maximum_rss_bytes: current_usage.maximum_rss_bytes,
            benchmark_minor_faults: current_usage.minor_faults,
            benchmark_major_faults: current_usage.major_faults,
        },
        host: host(),
    })
}

fn build_report(
    contract_id: &str,
    preflight: &Preflight,
    measurement: Measurement,
    retained: RetainedIdentities,
) -> Report {
    let input = &preflight.input;
    Report {
        schema: "pangopup-reference-production-qualification-reuse-v2".to_owned(),
        qualification_contract_id: contract_id.to_owned(),
        prior_build_contract_id: input.prior_build.contract_id.clone(),
        passed: true,
        identities: ReportIdentities {
            source: input.source.clone(),
            assembly_report: input.assembly_report.clone(),
            prior_build: PriorBuildReportIdentities {
                qualification_input: input.prior_build.qualification_input.clone(),
                build_command: input.prior_build.build_command.clone(),
                builder_executable: input.prior_build.builder_executable.clone(),
                builder_source_sha256: input.prior_build.builder_source_sha256.clone(),
                builder_stdout: input.prior_build.builder_stdout.clone(),
                builder_stderr: input.prior_build.builder_stderr.clone(),
                builder_heap_report: input.prior_build.builder_heap_report.clone(),
                builder_resource_log: input.prior_build.builder_resource_log.clone(),
                bundle_id: input.prior_build.bundle_id.clone(),
                manifest: input.prior_build.manifest.clone(),
                notice: input.prior_build.notice.clone(),
                reference_member: input.prior_build.reference_member.clone(),
            },
            prior_failure: PriorFailureReportIdentities {
                benchmark_executable: input.prior_failure.benchmark_executable.clone(),
                harness_source_sha256: input.prior_failure.harness_source_sha256.clone(),
                qualification_command: input.prior_failure.qualification_command.clone(),
                qualification_report: input.prior_failure.qualification_report.clone(),
                qualification_stderr: input.prior_failure.qualification_stderr.clone(),
                qualification_resource_log: input.prior_failure.qualification_resource_log.clone(),
            },
            replacement: ReplacementReportIdentities {
                benchmark_executable: input.replacement.benchmark_executable.clone(),
                harness_source_sha256: input.replacement.harness_source_sha256.clone(),
                source_inventory_sha256: input.replacement.source_inventory_sha256.clone(),
                corpus: input.replacement.corpus.clone(),
                rust_version: input.replacement.rust_version.clone(),
            },
            retained,
        },
        logical: measurement.logical,
        method: measurement.method,
        performance: measurement.performance,
        storage: measurement.storage,
        resources: measurement.resources,
        host: measurement.host,
        mmap_rss_interpretation: "RSS includes resident file-backed mmap pages and is not equivalent to Rust heap; peak heap is measured separately with the Rust global allocator.".to_owned(),
    }
}

fn revalidate_all(
    preflight: &mut Preflight,
    reuse: &mut HeldFile,
    executable: &mut HeldFile,
) -> Result<(), Failure> {
    revalidate_held(reuse)?;
    revalidate_held(&mut preflight.corpus)?;
    revalidate_held(executable)?;
    for held in preflight.evidence.files.values_mut() {
        revalidate_held(held)?;
    }
    // The parsed corpus must still be the exact authenticated buffer.
    if identity_bytes(authenticated_bytes(&preflight.corpus, MAX_SMALL)?)
        != preflight.corpus.identity
    {
        return Err(Failure::measurement(
            "CORPUS_MUTATION",
            "corpus buffer identity changed",
        ));
    }
    Ok(())
}

fn verify_cases(
    reader: &ReferenceBundleOpen,
    cases: &[Case],
    destination: &mut [u8],
) -> Result<(), Failure> {
    for case in cases {
        copy_case(reader, case, destination)?;
        if destination[..case.bases.len()] != case.bases {
            let _ = &case.id;
            return Err(Failure::measurement(
                "CONTEXT",
                "compatibility context mismatch",
            ));
        }
    }
    Ok(())
}

fn copy_case(
    reader: &ReferenceBundleOpen,
    case: &Case,
    destination: &mut [u8],
) -> Result<(), Failure> {
    reader
        .copy_window(
            case.contig,
            case.start,
            &mut destination[..case.bases.len()],
        )
        .map_err(|_| Failure::measurement("WINDOW", "window copy failed"))
}

fn parse_cases(bytes: &[u8]) -> Result<Vec<Case>, Failure> {
    if bytes.len() as u64 != CASES_BYTES || identity_bytes(bytes).sha256 != CASES_SHA256 {
        return Err(Failure::preflight(
            "CORPUS_IDENTITY",
            "corpus identity is invalid",
        ));
    }
    parse_case_projection(bytes)
}

fn parse_case_projection(bytes: &[u8]) -> Result<Vec<Case>, Failure> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Failure::preflight("CORPUS_FORMAT", "corpus is not UTF-8"))?;
    let mut cases = Vec::new();
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|_| Failure::preflight("CORPUS_FORMAT", "corpus JSON is invalid"))?;
        if !value
            .get("id")
            .and_then(|value| value.as_str())
            .is_some_and(|id| id.starts_with('M'))
        {
            continue;
        }
        let envelope: CaseEnvelope = serde_json::from_value(value)
            .map_err(|_| Failure::preflight("CORPUS_CASE", "corpus case is invalid"))?;
        cases.push(Case {
            id: envelope.id,
            contig: envelope
                .input
                .contig
                .parse()
                .map_err(|_| Failure::preflight("CORPUS_CASE", "corpus contig is invalid"))?,
            start: GenomicPosition::new(envelope.context.start_1based)
                .map_err(|_| Failure::preflight("CORPUS_CASE", "corpus start is invalid"))?,
            bases: envelope.context.bases.into_bytes(),
        });
    }
    if cases.len() != 14 {
        return Err(Failure::preflight(
            "CORPUS_COUNT",
            "corpus case count is invalid",
        ));
    }
    Ok(cases)
}

fn canonical<T: Serialize>(value: &T, name: &'static str) -> Result<Vec<u8>, Failure> {
    serde_jcs::to_vec(value).map_err(|_| Failure::preflight("JSON", name))
}

fn parse_canonical<T>(bytes: &[u8], name: &'static str) -> Result<T, Failure>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let value: T = serde_json::from_slice(bytes).map_err(|_| Failure::preflight("JSON", name))?;
    if canonical(&value, name)? != bytes {
        return Err(Failure::preflight("JSON_CANONICAL", name));
    }
    Ok(value)
}

fn parse_canonical_line<T>(bytes: &[u8], name: &'static str) -> Result<T, Failure>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    let object = bytes
        .strip_suffix(b"\n")
        .ok_or_else(|| Failure::preflight("JSON_LINE", name))?;
    parse_canonical(object, name)
}

fn validate_sha(value: &str) -> Result<(), Failure> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(Failure::preflight("SHA256", "digest is not canonical"))
    }
}

fn validate_identity(identity: &Identity) -> Result<(), Failure> {
    validate_sha(&identity.sha256)
}

fn require_identity(
    observed: &Identity,
    expected: &Identity,
    _name: &'static str,
) -> Result<(), Failure> {
    if observed == expected {
        Ok(())
    } else {
        Err(Failure::preflight(
            "IDENTITY_MISMATCH",
            "evidence identity mismatch",
        ))
    }
}

fn identity_bytes(bytes: &[u8]) -> Identity {
    Identity {
        bytes: bytes.len() as u64,
        sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
    }
}

fn contains_ascii(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn open_absolute_directory(path: &Path) -> Result<File, Failure> {
    if !path.is_absolute() {
        return Err(Failure::preflight("PATH", "path must be absolute"));
    }
    let root = rustix::fs::open(
        "/",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| Failure::preflight("PATH", "root directory is unavailable"))?;
    let mut current = File::from(root);
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let next = rustix::fs::openat(
                    &current,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| Failure::preflight("PATH", "directory component is unavailable"))?;
                current = File::from(next);
            }
            _ => return Err(Failure::preflight("PATH", "path component is invalid")),
        }
    }
    Ok(current)
}

fn open_and_read_absolute(path: &Path, maximum: u64) -> Result<HeldFile, Failure> {
    let parent = path
        .parent()
        .ok_or_else(|| Failure::preflight("PATH", "input parent is missing"))?;
    let name = path
        .file_name()
        .ok_or_else(|| Failure::preflight("PATH", "input name is missing"))?;
    let directory = open_absolute_directory(parent)?;
    let mut held = open_relative_regular(&directory, Path::new(name), maximum)?;
    authenticate_held_buffered(&mut held, maximum)?;
    Ok(held)
}

fn open_relative_regular(root: &File, path: &Path, maximum: u64) -> Result<HeldFile, Failure> {
    let components: Vec<_> = path.components().collect();
    if components.is_empty()
        || components
            .iter()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Failure::preflight(
            "PATH",
            "relative evidence path is invalid",
        ));
    }
    let mut directory = root
        .try_clone()
        .map_err(|_| Failure::preflight("IO", "directory descriptor clone failed"))?;
    for component in &components[..components.len() - 1] {
        let Component::Normal(name) = component else {
            unreachable!();
        };
        let next = rustix::fs::openat(
            &directory,
            *name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::preflight("EVIDENCE", "evidence directory is unavailable"))?;
        directory = File::from(next);
    }
    let Component::Normal(name) = components[components.len() - 1] else {
        unreachable!();
    };
    let descriptor = rustix::fs::openat(
        &directory,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| Failure::preflight("EVIDENCE", "evidence file is unavailable"))?;
    held_from_file(File::from(descriptor), maximum)
}

fn open_current_executable() -> Result<HeldFile, Failure> {
    // `/proc/self/exe` itself is a kernel-provided magic link, so opening it
    // follows that one link and immediately retains the resulting regular-file
    // descriptor. No later operation resolves the executable pathname.
    let file = File::open("/proc/self/exe")
        .map_err(|_| Failure::preflight("EXECUTABLE", "current executable is unavailable"))?;
    let mut held = held_from_file(file, MAX_EXECUTABLE)?;
    authenticate_held(&mut held)?;
    Ok(held)
}

fn held_from_file(file: File, maximum: u64) -> Result<HeldFile, Failure> {
    let metadata = file
        .metadata()
        .map_err(|_| Failure::preflight("EVIDENCE", "evidence metadata is unavailable"))?;
    if !metadata.file_type().is_file() || metadata.len() > maximum {
        return Err(Failure::preflight(
            "EVIDENCE",
            "evidence must be a bounded regular file",
        ));
    }
    Ok(HeldFile {
        file,
        stat: HeldStat {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
        },
        identity: Identity {
            bytes: metadata.len(),
            sha256: String::new(),
        },
        authenticated_bytes: None,
        descriptor_read_passes: 0,
    })
}

fn authenticate_held(held: &mut HeldFile) -> Result<(), Failure> {
    let identity = hash_descriptor(&held.file, held.stat.size)?;
    held.descriptor_read_passes = held.descriptor_read_passes.saturating_add(1);
    verify_stat(held)?;
    held.identity = identity;
    Ok(())
}

fn authenticate_held_buffered(held: &mut HeldFile, maximum: u64) -> Result<(), Failure> {
    use std::os::unix::fs::FileExt;
    if held.stat.size > maximum || held.stat.size > usize::MAX as u64 {
        return Err(Failure::preflight("EVIDENCE", "evidence exceeds its bound"));
    }
    let mut bytes = vec![0_u8; held.stat.size as usize];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let read = held
            .file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|_| Failure::preflight("EVIDENCE", "evidence read failed"))?;
        if read == 0 {
            return Err(Failure::preflight("EVIDENCE", "evidence was truncated"));
        }
        offset += read;
    }
    held.descriptor_read_passes = held.descriptor_read_passes.saturating_add(1);
    verify_stat(held)?;
    held.identity = identity_bytes(&bytes);
    held.authenticated_bytes = Some(bytes);
    Ok(())
}

fn revalidate_held(held: &mut HeldFile) -> Result<(), Failure> {
    verify_stat(held)?;
    let identity = hash_descriptor(&held.file, held.stat.size)?;
    held.descriptor_read_passes = held.descriptor_read_passes.saturating_add(1);
    verify_stat(held)?;
    if identity != held.identity {
        return Err(Failure::measurement(
            "EVIDENCE_MUTATION",
            "held evidence changed",
        ));
    }
    Ok(())
}

fn verify_stat(held: &HeldFile) -> Result<(), Failure> {
    let metadata = held
        .file
        .metadata()
        .map_err(|_| Failure::measurement("EVIDENCE_MUTATION", "held evidence metadata failed"))?;
    if metadata.file_type().is_file()
        && metadata.dev() == held.stat.device
        && metadata.ino() == held.stat.inode
        && metadata.len() == held.stat.size
    {
        Ok(())
    } else {
        Err(Failure::measurement(
            "EVIDENCE_MUTATION",
            "held evidence changed",
        ))
    }
}

fn hash_descriptor(file: &File, expected_size: u64) -> Result<Identity, Failure> {
    use std::os::unix::fs::FileExt;
    let mut hash = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    while offset < expected_size {
        let wanted = usize::try_from((expected_size - offset).min(buffer.len() as u64))
            .map_err(|_| Failure::preflight("IDENTITY", "evidence size is invalid"))?;
        let read = file
            .read_at(&mut buffer[..wanted], offset)
            .map_err(|_| Failure::preflight("IDENTITY", "evidence read failed"))?;
        if read == 0 {
            return Err(Failure::preflight("IDENTITY", "evidence was truncated"));
        }
        hash.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or_else(|| Failure::preflight("IDENTITY", "evidence size overflow"))?;
    }
    Ok(Identity {
        bytes: offset,
        sha256: format!("sha256:{:x}", hash.finalize()),
    })
}

fn authenticated_bytes(held: &HeldFile, maximum: u64) -> Result<&[u8], Failure> {
    if held.stat.size > maximum {
        return Err(Failure::preflight("EVIDENCE", "evidence exceeds its bound"));
    }
    held.authenticated_bytes
        .as_deref()
        .ok_or_else(|| Failure::preflight("EVIDENCE", "evidence buffer is unavailable"))
}

fn ensure_child_absent(parent: &File, name: &OsStr) -> Result<(), Failure> {
    match rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Err(Failure::preflight(
            "ALREADY_EXISTS",
            "qualification output already exists",
        )),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
        Err(_) => Err(Failure::preflight(
            "OUTPUT",
            "qualification output cannot be inspected",
        )),
    }
}

struct QualificationStage {
    directory: File,
    parent_path: PathBuf,
    name: OsString,
    path: PathBuf,
    published: bool,
}

impl QualificationStage {
    fn create(parent: &File, parent_path: &Path, contract: &str) -> Result<Self, Failure> {
        for counter in 0..1024_u64 {
            let name = OsString::from(format!(
                ".{contract}.qualification-stage-{}-{counter}",
                std::process::id()
            ));
            match rustix::fs::mkdirat(parent, &name, rustix::fs::Mode::from(0o700)) {
                Ok(()) => {
                    let descriptor = rustix::fs::openat(
                        parent,
                        &name,
                        rustix::fs::OFlags::RDONLY
                            | rustix::fs::OFlags::DIRECTORY
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::CLOEXEC,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(|_| Failure::preflight("STAGE", "private stage cannot be opened"))?;
                    return Ok(Self {
                        directory: File::from(descriptor),
                        parent_path: parent_path.to_owned(),
                        path: parent_path.join(&name),
                        name,
                        published: false,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(_) => {
                    return Err(Failure::preflight(
                        "STAGE",
                        "private stage cannot be created",
                    ));
                }
            }
        }
        Err(Failure::preflight(
            "STAGE",
            "private stage name is unavailable",
        ))
    }

    fn mkdir(&self, name: &str, mode: u32) -> Result<(), Failure> {
        rustix::fs::mkdirat(&self.directory, name, rustix::fs::Mode::from(mode))
            .map_err(|_| Failure::preflight("STAGE", "stage directory creation failed"))
    }

    fn member_parent(&self, path: &str) -> Result<(File, OsString), Failure> {
        let path = Path::new(path);
        let name = path
            .file_name()
            .ok_or_else(|| Failure::preflight("STAGE", "stage member name is invalid"))?
            .to_owned();
        match path.parent() {
            Some(parent) if parent == Path::new("candidate") => {
                let descriptor = rustix::fs::openat(
                    &self.directory,
                    "candidate",
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| Failure::preflight("STAGE", "candidate stage is unavailable"))?;
                Ok((File::from(descriptor), name))
            }
            Some(parent) if parent.as_os_str().is_empty() => Ok((
                self.directory
                    .try_clone()
                    .map_err(|_| Failure::preflight("STAGE", "stage descriptor clone failed"))?,
                name,
            )),
            _ => Err(Failure::preflight("STAGE", "stage member path is invalid")),
        }
    }

    fn write_file(&self, path: &str, bytes: &[u8], mode: u32) -> Result<(), Failure> {
        let (parent, name) = self.member_parent(path)?;
        let descriptor = rustix::fs::openat(
            &parent,
            &name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from(mode),
        )
        .map_err(|_| Failure::preflight("STAGE", "stage member creation failed"))?;
        let mut file = File::from(descriptor);
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Failure::preflight("STAGE", "stage member write failed"))
    }

    fn replace_file(&self, path: &str, bytes: &[u8], mode: u32) -> Result<(), Failure> {
        let (parent, name) = self.member_parent(path)?;
        let descriptor = rustix::fs::openat(
            &parent,
            &name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::TRUNC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::publication("STAGE", "stage member replacement failed"))?;
        let mut file = File::from(descriptor);
        file.set_permissions(Permissions::from_mode(mode))
            .and_then(|_| file.write_all(bytes))
            .and_then(|_| file.sync_all())
            .map_err(|_| Failure::publication("STAGE", "stage member replacement failed"))
    }

    fn copy_held(&self, path: &str, source: &HeldFile, mode: u32) -> Result<(), Failure> {
        use std::os::unix::fs::FileExt;
        let (parent, name) = self.member_parent(path)?;
        let descriptor = rustix::fs::openat(
            &parent,
            &name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from(mode),
        )
        .map_err(|_| Failure::preflight("STAGE", "candidate creation failed"))?;
        let mut output = File::from(descriptor);
        let mut offset = 0_u64;
        let mut buffer = vec![0_u8; 1024 * 1024];
        while offset < source.stat.size {
            let wanted = ((source.stat.size - offset) as usize).min(buffer.len());
            let read = source
                .file
                .read_at(&mut buffer[..wanted], offset)
                .map_err(|_| Failure::preflight("STAGE", "candidate source read failed"))?;
            if read == 0 {
                return Err(Failure::preflight(
                    "STAGE",
                    "candidate source was truncated",
                ));
            }
            output
                .write_all(&buffer[..read])
                .map_err(|_| Failure::preflight("STAGE", "candidate write failed"))?;
            offset += read as u64;
        }
        output
            .sync_all()
            .map_err(|_| Failure::preflight("STAGE", "candidate sync failed"))
    }

    fn set_member_mode(&self, path: &str, mode: u32) -> Result<(), Failure> {
        let (parent, name) = self.member_parent(path)?;
        let descriptor = rustix::fs::openat(
            &parent,
            &name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::publication("STAGE", "stage member is unavailable"))?;
        let file = File::from(descriptor);
        file.set_permissions(Permissions::from_mode(mode))
            .and_then(|_| file.sync_all())
            .map_err(|_| Failure::publication("STAGE", "stage mode update failed"))
    }

    fn seal_success(&self) -> Result<(), Failure> {
        for path in [
            "reuse-input.json",
            "command.json",
            "qualification-report.json",
            "qualification-resource.json",
            "qualification.stderr.log",
            "candidate/pangopup-reference-benchmark.rs",
        ] {
            self.set_member_mode(path, 0o444)?;
        }
        self.set_member_mode("candidate/pangopup-reference-benchmark", 0o555)?;
        self.sync_candidate()?;
        self.directory
            .sync_all()
            .map_err(|_| Failure::publication("SYNC", "stage sync failed"))
    }

    fn sync_candidate(&self) -> Result<(), Failure> {
        let descriptor = rustix::fs::openat(
            &self.directory,
            "candidate",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::publication("SYNC", "candidate directory is unavailable"))?;
        File::from(descriptor)
            .sync_all()
            .map_err(|_| Failure::publication("SYNC", "candidate directory sync failed"))
    }

    fn publish_success(&mut self, parent: &File, output: &OsStr) -> Result<(), Failure> {
        self.refresh_location(parent)?;
        let private_name = self.name.clone();
        rename_noreplace(parent, &private_name, output)?;
        self.name = output.to_owned();
        self.path = self.parent_path.join(output);
        self.published = true;
        if parent.sync_all().is_err() {
            // Restore the private name so failure evidence can still be
            // completed and no unsynced success is mislabeled as accepted.
            rename_noreplace(parent, &self.name, &private_name)?;
            self.name = private_name;
            self.path = self.parent_path.join(&self.name);
            self.published = false;
            parent
                .sync_all()
                .map_err(|_| Failure::publication("SYNC", "publication rollback sync failed"))?;
            return Err(Failure::publication("SYNC", "success parent sync failed"));
        }
        Ok(())
    }

    fn publish_failure(&mut self, parent: &File, contract: &str) -> Result<OsString, Failure> {
        self.refresh_location(parent)?;
        self.directory
            .sync_all()
            .map_err(|_| Failure::publication("SYNC", "failure stage sync failed"))?;
        for counter in 0..1024_u64 {
            let private_name = self.name.clone();
            let failure_name = OsString::from(format!(
                ".{contract}.qualification-failed-{}-{counter}",
                std::process::id()
            ));
            match rename_noreplace(parent, &private_name, &failure_name) {
                Ok(()) => {
                    self.name = failure_name.clone();
                    self.path = self.parent_path.join(&failure_name);
                    self.published = true;
                    if parent.sync_all().is_err() {
                        rename_noreplace(parent, &self.name, &private_name)?;
                        self.name = private_name;
                        self.path = self.parent_path.join(&self.name);
                        self.published = false;
                        parent.sync_all().map_err(|_| {
                            Failure::publication("SYNC", "failure rollback sync failed")
                        })?;
                        return Err(Failure::publication("SYNC", "failure parent sync failed"));
                    }
                    return Ok(failure_name);
                }
                Err(error) if error.code == "ALREADY_EXISTS" => continue,
                Err(error) => return Err(error),
            }
        }
        Err(Failure::publication(
            "ALREADY_EXISTS",
            "failure receipt name is unavailable",
        ))
    }

    fn refresh_location(&mut self, parent: &File) -> Result<(), Failure> {
        let held = self
            .directory
            .metadata()
            .map_err(|_| Failure::publication("STAGE_LOCATION", "held stage metadata failed"))?;
        let matches = directory_names(parent)?
            .into_iter()
            .filter_map(|name| {
                let stat = rustix::fs::statat(parent, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .ok()?;
                (stat.st_dev == held.dev() && stat.st_ino == held.ino()).then_some(name)
            })
            .collect::<Vec<_>>();
        let [name] = matches.as_slice() else {
            return Err(Failure::publication(
                "STAGE_LOCATION",
                "held stage has no unique current parent entry",
            ));
        };
        self.name = name.clone();
        self.path = self.parent_path.join(name);
        let after = self
            .directory
            .metadata()
            .map_err(|_| Failure::publication("STAGE_LOCATION", "held stage metadata failed"))?;
        if after.dev() != held.dev() || after.ino() != held.ino() {
            return Err(Failure::publication(
                "STAGE_LOCATION",
                "held stage identity changed during location binding",
            ));
        }
        Ok(())
    }
}

fn rename_noreplace(parent: &File, from: &OsStr, to: &OsStr) -> Result<(), Failure> {
    rustix::fs::renameat_with(parent, from, parent, to, rustix::fs::RenameFlags::NOREPLACE).map_err(
        |error| {
            if error == rustix::io::Errno::EXIST || error == rustix::io::Errno::NOTEMPTY {
                Failure::publication("ALREADY_EXISTS", "publication destination already exists")
            } else {
                Failure::publication("PUBLICATION", "atomic publication failed")
            }
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn seal_and_publish_failure(
    stage: &mut QualificationStage,
    parent: &File,
    contract: &str,
    prior_contract: &str,
    error: Failure,
    timing: FailureTiming,
    identities: FailureIdentities<'_>,
    reuse_bytes: &[u8],
    command_bytes: &[u8],
    executable: &HeldFile,
) -> Result<(), Failure> {
    let result = try_seal_and_publish_failure(
        stage,
        parent,
        contract,
        prior_contract,
        &error,
        timing,
        identities,
        reuse_bytes,
        command_bytes,
        executable,
    );
    match result {
        Ok(()) => Ok(()),
        Err(failure) => {
            stage.refresh_location(parent).map_err(|location| {
                Failure::publication(
                    "STAGE_LOCATION",
                    format!(
                        "{}; failure sealing also failed: {}",
                        location.message, failure.message
                    ),
                )
            })?;
            Err(Failure::publication(
                "STAGE_PRESERVED",
                format!(
                    "{}; private stage preserved at {}",
                    failure.message,
                    stage.path.display()
                ),
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn try_seal_and_publish_failure(
    stage: &mut QualificationStage,
    parent: &File,
    contract: &str,
    prior_contract: &str,
    error: &Failure,
    timing: FailureTiming,
    identities: FailureIdentities<'_>,
    reuse_bytes: &[u8],
    command_bytes: &[u8],
    executable: &HeldFile,
) -> Result<(), Failure> {
    let retained_report = if error.phase == FailurePhase::Publication {
        read_complete_unpublished_report(stage).ok()
    } else {
        None
    };
    reset_stage_contents(stage)?;
    stage.write_file("reuse-input.json", reuse_bytes, 0o600)?;
    stage.write_file("command.json", command_bytes, 0o600)?;
    stage.mkdir("candidate", 0o700)?;
    stage.copy_held("candidate/pangopup-reference-benchmark", executable, 0o500)?;
    stage.write_file(
        "candidate/pangopup-reference-benchmark.rs",
        HARNESS_SOURCE,
        0o600,
    )?;
    let (report, report_state) = retained_report
        .map(|bytes| (bytes, ReportState::CompleteUnpublished))
        .unwrap_or_else(|| (Vec::new(), ReportState::Empty));
    stage.write_file("qualification-report.json", &report, 0o600)?;
    let current = usage().unwrap_or(timing.usage_before);
    let outcome = match error.phase {
        FailurePhase::Preflight => ResourceOutcome::PreflightFailed,
        FailurePhase::Measurement => ResourceOutcome::MeasurementFailed,
        FailurePhase::Publication => ResourceOutcome::PublicationFailed,
    };
    let resource = canonical(
        &resource_receipt(
            timing.started,
            timing.started_at,
            timing.usage_before,
            current,
            outcome,
        )?,
        "resource",
    )?;
    let stderr = failure_line(error)?;
    stage.write_file("qualification-resource.json", &resource, 0o600)?;
    stage.write_file("qualification.stderr.log", &stderr, 0o600)?;
    let evidence = FailureEvidence {
        reuse_input: identities.reuse.clone(),
        command: identities.command.clone(),
        resource: identity_bytes(&resource),
        stderr: identity_bytes(&stderr),
        report: identity_bytes(&report),
        benchmark_executable: identities.executable.clone(),
        harness_source: identities.harness.clone(),
    };
    let receipt = FailureReceipt {
        schema: "pangopup-reference-qualification-failure-v2".to_owned(),
        qualification_contract_id: format!("sha256:{contract}"),
        prior_build_contract_id: prior_contract.to_owned(),
        phase: error.phase,
        code: error.code.to_owned(),
        message: error.message.clone(),
        report_state,
        evidence,
    };
    stage.write_file("failure.json", &canonical(&receipt, "failure")?, 0o600)?;
    seal_failure(stage)?;
    validate_failure_stage(stage)?;
    stage.publish_failure(parent, contract)?;
    Ok(())
}

fn read_complete_unpublished_report(stage: &QualificationStage) -> Result<Vec<u8>, Failure> {
    let mut held = open_relative_regular(
        &stage.directory,
        Path::new("qualification-report.json"),
        MAX_SMALL,
    )?;
    authenticate_held_buffered(&mut held, MAX_SMALL)?;
    let bytes = authenticated_bytes(&held, MAX_SMALL)?.to_vec();
    let report: Report = parse_canonical(&bytes, "report")?;
    if !report.passed {
        return Err(Failure::publication(
            "REPORT",
            "unpublished report did not pass",
        ));
    }
    Ok(bytes)
}

fn reset_stage_contents(stage: &QualificationStage) -> Result<(), Failure> {
    clear_directory_contents(&stage.directory)
}

fn directory_names(directory: &File) -> Result<Vec<OsString>, Failure> {
    let mut stream = rustix::fs::Dir::read_from(directory)
        .map_err(|_| Failure::publication("STAGE", "descriptor inventory failed"))?;
    let mut names = Vec::new();
    while let Some(entry) = stream.read() {
        let entry =
            entry.map_err(|_| Failure::publication("STAGE", "descriptor inventory failed"))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        if bytes.is_empty() || bytes.contains(&b'/') {
            return Err(Failure::publication(
                "STAGE",
                "descriptor inventory contained an invalid member",
            ));
        }
        names.push(OsString::from_vec(bytes.to_vec()));
    }
    names.sort();
    Ok(names)
}

fn clear_directory_contents(directory: &File) -> Result<(), Failure> {
    for name in directory_names(directory)? {
        let stat = rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Failure::publication("STAGE", "stage reset metadata failed"))?;
        let file_type = rustix::fs::FileType::from_raw_mode(stat.st_mode);
        if file_type.is_dir() {
            let child = File::from(
                rustix::fs::openat(
                    directory,
                    &name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|_| Failure::publication("STAGE", "stage child open failed"))?,
            );
            let held = child
                .metadata()
                .map_err(|_| Failure::publication("STAGE", "stage child metadata failed"))?;
            if held.dev() != stat.st_dev || held.ino() != stat.st_ino {
                return Err(Failure::publication(
                    "STAGE",
                    "stage child changed before reset",
                ));
            }
            clear_directory_contents(&child)?;
            let current =
                rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(|_| {
                        Failure::publication("STAGE", "stage child revalidation failed")
                    })?;
            if current.st_dev != held.dev() || current.st_ino != held.ino() {
                return Err(Failure::publication(
                    "STAGE",
                    "stage child changed during reset",
                ));
            }
            rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::REMOVEDIR)
                .map_err(|_| Failure::publication("STAGE", "stage directory reset failed"))?;
        } else {
            rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::empty())
                .map_err(|_| Failure::publication("STAGE", "stage member reset failed"))?;
        }
    }
    Ok(())
}

fn failure_line(error: &Failure) -> Result<Vec<u8>, Failure> {
    failure_line_parts(error.code, &error.message)
}

fn failure_line_parts(code: &str, message: &str) -> Result<Vec<u8>, Failure> {
    if code.len() > 64 || message.len() > 256 || !code.is_ascii() || !message.is_ascii() {
        return Err(Failure::publication(
            "FAILURE_RECEIPT",
            "failure text is invalid",
        ));
    }
    Ok(format!("reference qualification failed: {}: {}\n", code, message).into_bytes())
}

fn seal_failure(stage: &QualificationStage) -> Result<(), Failure> {
    for path in [
        "reuse-input.json",
        "command.json",
        "qualification-report.json",
        "qualification-resource.json",
        "qualification.stderr.log",
        "candidate/pangopup-reference-benchmark.rs",
        "failure.json",
    ] {
        stage.set_member_mode(path, 0o444)?;
    }
    stage.set_member_mode("candidate/pangopup-reference-benchmark", 0o555)?;
    stage.sync_candidate()?;
    stage
        .directory
        .sync_all()
        .map_err(|_| Failure::publication("SYNC", "failure stage sync failed"))
}

fn stage_member_identity(
    stage: &QualificationStage,
    path: &str,
    maximum: u64,
) -> Result<Identity, Failure> {
    let held = open_relative_regular(&stage.directory, Path::new(path), maximum)?;
    hash_descriptor(&held.file, held.stat.size)
}

fn validate_success_stage(stage: &QualificationStage, report: &Report) -> Result<(), Failure> {
    let root = directory_names(&stage.directory)?;
    let expected_root = [
        "candidate",
        "command.json",
        "qualification-report.json",
        "qualification-resource.json",
        "qualification.stderr.log",
        "reuse-input.json",
    ]
    .map(OsString::from);
    if root != expected_root {
        return Err(Failure::publication(
            "STAGE_MEMBERS",
            "success member set is invalid",
        ));
    }
    let candidate_directory = File::from(
        rustix::fs::openat(
            &stage.directory,
            "candidate",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::publication("STAGE", "candidate inventory failed"))?,
    );
    let candidate = directory_names(&candidate_directory)?;
    if candidate
        != [
            "pangopup-reference-benchmark",
            "pangopup-reference-benchmark.rs",
        ]
        .map(OsString::from)
    {
        return Err(Failure::publication(
            "STAGE_MEMBERS",
            "candidate member set is invalid",
        ));
    }
    let expected = [
        (
            "reuse-input.json",
            &report.identities.retained.reuse_input,
            0o444,
        ),
        ("command.json", &report.identities.retained.command, 0o444),
        (
            "qualification-resource.json",
            &report.identities.retained.resource,
            0o444,
        ),
        (
            "qualification.stderr.log",
            &report.identities.retained.stderr,
            0o444,
        ),
        (
            "candidate/pangopup-reference-benchmark",
            &report.identities.retained.benchmark_executable,
            0o555,
        ),
        (
            "candidate/pangopup-reference-benchmark.rs",
            &report.identities.retained.harness_source,
            0o444,
        ),
    ];
    for (path, identity, mode) in expected {
        let held = open_relative_regular(&stage.directory, Path::new(path), MAX_EXECUTABLE)?;
        if hash_descriptor(&held.file, held.stat.size)? != *identity {
            return Err(Failure::publication(
                "STAGE_IDENTITY",
                "success member identity is invalid",
            ));
        }
        let metadata = held
            .file
            .metadata()
            .map_err(|_| Failure::publication("STAGE", "success member metadata failed"))?;
        if metadata.mode() & 0o777 != mode {
            return Err(Failure::publication(
                "STAGE_MODE",
                "success member mode is invalid",
            ));
        }
    }
    let mut report_held = open_relative_regular(
        &stage.directory,
        Path::new("qualification-report.json"),
        MAX_SMALL,
    )?;
    authenticate_held_buffered(&mut report_held, MAX_SMALL)?;
    let report_bytes = authenticated_bytes(&report_held, MAX_SMALL)?;
    let expected_report = canonical(report, "report")?;
    let _: Report = parse_canonical(report_bytes, "report")?;
    if report_bytes != expected_report {
        return Err(Failure::publication(
            "STAGE_REPORT",
            "success report bytes do not match the accepted report",
        ));
    }
    if report_held
        .file
        .metadata()
        .map_err(|_| Failure::publication("STAGE", "report metadata failed"))?
        .mode()
        & 0o777
        != 0o444
    {
        return Err(Failure::publication("STAGE_MODE", "report mode is invalid"));
    }
    Ok(())
}

fn validate_and_publish_success(
    stage: &mut QualificationStage,
    parent: &File,
    output: &OsStr,
    report: &Report,
) -> Result<(), Failure> {
    validate_success_stage(stage, report)?;
    stage.publish_success(parent, output)
}

fn validate_failure_stage(stage: &QualificationStage) -> Result<(), Failure> {
    let root = directory_names(&stage.directory)?;
    if root
        != [
            "candidate",
            "command.json",
            "failure.json",
            "qualification-report.json",
            "qualification-resource.json",
            "qualification.stderr.log",
            "reuse-input.json",
        ]
        .map(OsString::from)
    {
        return Err(Failure::publication(
            "STAGE_MEMBERS",
            "failure member set is invalid",
        ));
    }
    let candidate_directory = File::from(
        rustix::fs::openat(
            &stage.directory,
            "candidate",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| Failure::publication("STAGE", "candidate inventory failed"))?,
    );
    let candidate = directory_names(&candidate_directory)?;
    if candidate
        != [
            "pangopup-reference-benchmark",
            "pangopup-reference-benchmark.rs",
        ]
        .map(OsString::from)
    {
        return Err(Failure::publication(
            "STAGE_MEMBERS",
            "failure candidate member set is invalid",
        ));
    }
    let receipt_bytes = stage_member_bytes(stage, "failure.json", MAX_SMALL)?;
    let receipt: FailureReceipt = parse_canonical(&receipt_bytes, "failure")?;
    let expected = [
        ("reuse-input.json", &receipt.evidence.reuse_input, 0o444),
        ("command.json", &receipt.evidence.command, 0o444),
        (
            "qualification-resource.json",
            &receipt.evidence.resource,
            0o444,
        ),
        ("qualification.stderr.log", &receipt.evidence.stderr, 0o444),
        ("qualification-report.json", &receipt.evidence.report, 0o444),
        (
            "candidate/pangopup-reference-benchmark",
            &receipt.evidence.benchmark_executable,
            0o555,
        ),
        (
            "candidate/pangopup-reference-benchmark.rs",
            &receipt.evidence.harness_source,
            0o444,
        ),
    ];
    for (path, identity, mode) in expected {
        validate_stage_member(stage, path, identity, mode)?;
    }
    let failure_identity = identity_bytes(&receipt_bytes);
    validate_stage_member(stage, "failure.json", &failure_identity, 0o444)?;
    let report = stage_member_bytes(stage, "qualification-report.json", MAX_SMALL)?;
    match receipt.report_state {
        ReportState::Empty if !report.is_empty() => {
            return Err(Failure::publication(
                "STAGE_REPORT",
                "failure report must be empty",
            ));
        }
        ReportState::CompleteUnpublished => {
            let parsed: Report = parse_canonical(&report, "report")?;
            if !parsed.passed || receipt.phase != FailurePhase::Publication {
                return Err(Failure::publication(
                    "STAGE_REPORT",
                    "unpublished report state is invalid",
                ));
            }
        }
        ReportState::Empty => {}
    }
    let resource: ResourceReceipt = parse_canonical(
        &stage_member_bytes(stage, "qualification-resource.json", MAX_SMALL)?,
        "resource",
    )?;
    let expected_outcome = match receipt.phase {
        FailurePhase::Preflight => ResourceOutcome::PreflightFailed,
        FailurePhase::Measurement => ResourceOutcome::MeasurementFailed,
        FailurePhase::Publication => ResourceOutcome::PublicationFailed,
    };
    if resource.outcome != expected_outcome {
        return Err(Failure::publication(
            "STAGE_RESOURCE",
            "failure resource outcome is invalid",
        ));
    }
    if stage_member_bytes(stage, "qualification.stderr.log", MAX_SMALL)?
        != failure_line_parts(&receipt.code, &receipt.message)?
    {
        return Err(Failure::publication(
            "STAGE_STDERR",
            "failure stderr is invalid",
        ));
    }
    Ok(())
}

fn validate_stage_member(
    stage: &QualificationStage,
    path: &str,
    identity: &Identity,
    mode: u32,
) -> Result<(), Failure> {
    let held = open_relative_regular(&stage.directory, Path::new(path), MAX_EXECUTABLE)?;
    if hash_descriptor(&held.file, held.stat.size)? != *identity {
        return Err(Failure::publication(
            "STAGE_IDENTITY",
            "stage member identity is invalid",
        ));
    }
    if held
        .file
        .metadata()
        .map_err(|_| Failure::publication("STAGE", "stage member metadata failed"))?
        .mode()
        & 0o777
        != mode
    {
        return Err(Failure::publication(
            "STAGE_MODE",
            "stage member mode is invalid",
        ));
    }
    Ok(())
}

fn stage_member_bytes(
    stage: &QualificationStage,
    path: &str,
    maximum: u64,
) -> Result<Vec<u8>, Failure> {
    let mut held = open_relative_regular(&stage.directory, Path::new(path), maximum)?;
    authenticate_held_buffered(&mut held, maximum)?;
    Ok(authenticated_bytes(&held, maximum)?.to_vec())
}

fn resource_receipt(
    started: u64,
    started_at: Instant,
    before: Usage,
    after: Usage,
    outcome: ResourceOutcome,
) -> Result<ResourceReceipt, Failure> {
    Ok(ResourceReceipt {
        schema: "pangopup-reference-qualification-resource-v2".to_owned(),
        started_unix_seconds: started,
        elapsed_ns: nanos(started_at.elapsed().as_nanos())?,
        maximum_rss_bytes: after.maximum_rss_bytes,
        minor_faults: after.minor_faults.saturating_sub(before.minor_faults),
        major_faults: after.major_faults.saturating_sub(before.major_faults),
        user_cpu_ns: after.user_cpu_ns.saturating_sub(before.user_cpu_ns),
        system_cpu_ns: after.system_cpu_ns.saturating_sub(before.system_cpu_ns),
        outcome,
    })
}

fn read_time_report(bytes: &[u8], expected_exit: u64) -> Result<Usage, Failure> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Failure::preflight("RESOURCE_LOG", "resource log is not UTF-8"))?;
    let value = |label: &str| -> Result<u64, Failure> {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(label))
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(|| Failure::preflight("RESOURCE_LOG", "resource field is missing"))
    };
    if value("Exit status:")? != expected_exit {
        return Err(Failure::preflight(
            "RESOURCE_LOG",
            "resource exit status is invalid",
        ));
    }
    Ok(Usage {
        maximum_rss_bytes: value("Maximum resident set size (kbytes):")?
            .checked_mul(1024)
            .ok_or_else(|| Failure::preflight("RESOURCE_LOG", "resource RSS overflow"))?,
        minor_faults: value("Minor (reclaiming a frame) page faults:")?,
        major_faults: value("Major (requiring I/O) page faults:")?,
        user_cpu_ns: 0,
        system_cpu_ns: 0,
    })
}

fn usage() -> Result<Usage, Failure> {
    let mut value = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied structure on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, value.as_mut_ptr()) } != 0 {
        return Err(Failure::measurement("RESOURCE", "getrusage failed"));
    }
    // SAFETY: the successful call initialized value.
    let value = unsafe { value.assume_init() };
    Ok(Usage {
        maximum_rss_bytes: u64::try_from(value.ru_maxrss)
            .map_err(|_| Failure::measurement("RESOURCE", "RSS is invalid"))?
            .checked_mul(1024)
            .ok_or_else(|| Failure::measurement("RESOURCE", "RSS overflow"))?,
        minor_faults: u64::try_from(value.ru_minflt)
            .map_err(|_| Failure::measurement("RESOURCE", "minor faults are invalid"))?,
        major_faults: u64::try_from(value.ru_majflt)
            .map_err(|_| Failure::measurement("RESOURCE", "major faults are invalid"))?,
        user_cpu_ns: timeval_ns(value.ru_utime)?,
        system_cpu_ns: timeval_ns(value.ru_stime)?,
    })
}

fn timeval_ns(value: libc::timeval) -> Result<u64, Failure> {
    let seconds = u64::try_from(value.tv_sec)
        .map_err(|_| Failure::measurement("RESOURCE", "CPU time is invalid"))?;
    let micros = u64::try_from(value.tv_usec)
        .map_err(|_| Failure::measurement("RESOURCE", "CPU time is invalid"))?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|total| total.checked_add(micros * 1_000))
        .ok_or_else(|| Failure::measurement("RESOURCE", "CPU time overflow"))
}

fn zstd_size(file: &File, size: u64) -> Result<u64, Failure> {
    let mut encoder = zstd::stream::Encoder::new(CountingSink(0), 9)
        .map_err(|_| Failure::measurement("ZSTD", "zstd encoder failed"))?;
    encoder
        .include_checksum(true)
        .and_then(|_| encoder.include_contentsize(true))
        .and_then(|_| encoder.include_dictid(false))
        .and_then(|_| encoder.long_distance_matching(false))
        .and_then(|_| encoder.multithread(0))
        .and_then(|_| encoder.set_pledged_src_size(Some(size)))
        .map_err(|_| Failure::measurement("ZSTD", "zstd parameters failed"))?;
    let mut input = file
        .try_clone()
        .map_err(|_| Failure::measurement("ZSTD", "reference descriptor clone failed"))?;
    input
        .seek(SeekFrom::Start(0))
        .map_err(|_| Failure::measurement("ZSTD", "reference descriptor seek failed"))?;
    std::io::copy(&mut input, &mut encoder)
        .map_err(|_| Failure::measurement("ZSTD", "zstd encode failed"))?;
    Ok(encoder
        .finish()
        .map_err(|_| Failure::measurement("ZSTD", "zstd finish failed"))?
        .0)
}

struct CountingSink(u64);
impl Write for CountingSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("compressed size overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn host() -> Host {
    let read = |path: &str| {
        fs::read_to_string(path)
            .unwrap_or_else(|_| "unavailable".to_owned())
            .trim()
            .to_owned()
    };
    let cpuinfo = read("/proc/cpuinfo");
    let cpu = cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .unwrap_or("unavailable")
        .to_owned();
    Host {
        hostname: read("/etc/hostname"),
        kernel: read("/proc/sys/kernel/osrelease"),
        cpu,
        os: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    }
}

fn nanos(value: u128) -> Result<u64, Failure> {
    u64::try_from(value).map_err(|_| Failure::measurement("DURATION", "duration overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    struct Temp(PathBuf);
    impl Temp {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "pangopup-reference-v2-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("temporary directory");
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ident(label: &str) -> Identity {
        identity_bytes(label.as_bytes())
    }

    fn sample_report() -> Report {
        let i = ident("i");
        Report {
            schema: "pangopup-reference-production-qualification-reuse-v2".into(),
            qualification_contract_id: i.sha256.clone(),
            prior_build_contract_id: i.sha256.clone(),
            passed: true,
            identities: ReportIdentities {
                source: i.clone(),
                assembly_report: i.clone(),
                prior_build: PriorBuildReportIdentities {
                    qualification_input: i.clone(),
                    build_command: i.clone(),
                    builder_executable: i.clone(),
                    builder_source_sha256: i.sha256.clone(),
                    builder_stdout: i.clone(),
                    builder_stderr: i.clone(),
                    builder_heap_report: i.clone(),
                    builder_resource_log: i.clone(),
                    bundle_id: i.sha256.clone(),
                    manifest: i.clone(),
                    notice: i.clone(),
                    reference_member: i.clone(),
                },
                prior_failure: PriorFailureReportIdentities {
                    benchmark_executable: i.clone(),
                    harness_source_sha256: i.sha256.clone(),
                    qualification_command: i.clone(),
                    qualification_report: i.clone(),
                    qualification_stderr: i.clone(),
                    qualification_resource_log: i.clone(),
                },
                replacement: ReplacementReportIdentities {
                    benchmark_executable: i.clone(),
                    harness_source_sha256: i.sha256.clone(),
                    source_inventory_sha256: i.sha256.clone(),
                    corpus: i.clone(),
                    rust_version: "rustc test".into(),
                },
                retained: RetainedIdentities {
                    reuse_input: i.clone(),
                    command: i.clone(),
                    resource: i.clone(),
                    stderr: i.clone(),
                    benchmark_executable: i.clone(),
                    harness_source: i.clone(),
                },
            },
            logical: Logical {
                total_bases: 1,
                sequence_set_sha256: i.sha256.clone(),
                extra_record_count: 0,
                extra_accessions_sha256: i.sha256.clone(),
                contexts_verified: 14,
            },
            method: Method {
                rounds: 5,
                warmups_per_round: 20,
                operations_per_round: 10_000,
                quantile: "nearest-rank".into(),
            },
            performance: Performance {
                open_ns: vec![1],
                round_p50_ns: vec![1],
                round_p95_ns: vec![1],
                headline_p50_ns: 1,
                headline_p95_ns: 1,
                allocation_calls: 0,
                allocation_bytes: 0,
                unique_dense_pages: vec![1],
                per_case_page_count_sum: 1,
            },
            storage: Storage {
                installed_bundle_bytes: 1,
                reference_member_bytes: 1,
                pinned_zstandard_bytes: 1,
                pinned_zstandard: "test".into(),
            },
            resources: Resources {
                builder_peak_heap_bytes: 1,
                open_peak_heap_bytes: 1,
                dense_bytes_read_during_open: 0,
                builder_maximum_rss_bytes: 1,
                builder_minor_faults: 1,
                builder_major_faults: 0,
                benchmark_maximum_rss_bytes: 1,
                benchmark_minor_faults: 1,
                benchmark_major_faults: 0,
            },
            host: Host {
                hostname: "host".into(),
                kernel: "kernel".into(),
                cpu: "cpu".into(),
                os: "linux".into(),
                architecture: "x86_64".into(),
            },
            mmap_rss_interpretation: "test".into(),
        }
    }

    #[test]
    fn exact_pinned_corpus_projects_all_fourteen_model_contexts() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/pangolin-compat-v1/cases.jsonl"
        ));
        let cases = parse_cases(bytes).expect("exact pinned corpus");
        assert_eq!(cases.len(), 14);
        assert_eq!(cases[0].id, "M01-snv-cd4-precomputed");
        assert_eq!(cases[0].start.get(), 6_796_251);
        assert_eq!(cases[13].id, "M14-deletion-ref100-overlap");
        assert_eq!(cases[13].bases.len(), 10_200);
    }

    fn projected_corpus(first: &str) -> Vec<u8> {
        let mut lines = vec![first.to_owned()];
        for index in 2..=14 {
            lines.push(format!(
                r#"{{"id":"M{index:02}","input":{{"contig":"chr1","ignored":true}},"context":{{"start_1based":1,"bases":"A","ignored":{{}}}},"ignored":[]}}"#
            ));
        }
        format!("{}\n", lines.join("\n")).into_bytes()
    }

    #[test]
    fn projection_accepts_unrelated_fields_and_rejects_consumed_field_drift() {
        let valid = projected_corpus(
            r#"{"id":"M01","input":{"contig":"chr1","ignored":true},"context":{"start_1based":1,"bases":"A","ignored":{}},"ignored":[]}"#,
        );
        assert_eq!(
            parse_case_projection(&valid)
                .expect("unrelated fields")
                .len(),
            14
        );
        let missing =
            projected_corpus(r#"{"id":"M01","input":{},"context":{"start_1based":1,"bases":"A"}}"#);
        assert_eq!(
            parse_case_projection(&missing)
                .expect_err("missing consumed field")
                .code,
            "CORPUS_CASE"
        );
        let mistyped = projected_corpus(
            r#"{"id":"M01","input":{"contig":"chr1"},"context":{"start_1based":"one","bases":"A"}}"#,
        );
        assert_eq!(
            parse_case_projection(&mistyped)
                .expect_err("mistyped consumed field")
                .code,
            "CORPUS_CASE"
        );
    }

    #[test]
    fn report_and_failure_receipts_are_closed_and_canonical() {
        let report = sample_report();
        let bytes = canonical(&report, "report").expect("canonical report");
        let _: Report = parse_canonical(&bytes, "report").expect("closed report");
        let text = String::from_utf8(bytes).expect("UTF-8");
        let top_unknown = text.replacen("{", "{\"unknown\":0,", 1);
        assert!(serde_json::from_str::<Report>(&top_unknown).is_err());
        let nested_unknown = text.replacen("\"identities\":{", "\"identities\":{\"unknown\":0,", 1);
        assert!(serde_json::from_str::<Report>(&nested_unknown).is_err());
        let retained_unknown = text.replacen("\"retained\":{", "\"retained\":{\"unknown\":0,", 1);
        assert!(serde_json::from_str::<Report>(&retained_unknown).is_err());
        let duplicate = text.replacen("\"passed\":true", "\"passed\":true,\"passed\":true", 1);
        assert!(serde_json::from_str::<Report>(&duplicate).is_err());

        let i = ident("evidence");
        let failure = FailureReceipt {
            schema: "pangopup-reference-qualification-failure-v2".into(),
            qualification_contract_id: i.sha256.clone(),
            prior_build_contract_id: i.sha256.clone(),
            phase: FailurePhase::Measurement,
            code: "TEST".into(),
            message: "test".into(),
            report_state: ReportState::Empty,
            evidence: FailureEvidence {
                reuse_input: i.clone(),
                command: i.clone(),
                resource: i.clone(),
                stderr: i.clone(),
                report: i.clone(),
                benchmark_executable: i.clone(),
                harness_source: i,
            },
        };
        let bytes = canonical(&failure, "failure").expect("canonical failure");
        let _: FailureReceipt = parse_canonical(&bytes, "failure").expect("closed failure");
        let text = String::from_utf8(bytes).expect("UTF-8");
        assert!(
            serde_json::from_str::<FailureReceipt>(&text.replacen(
                "\"evidence\":{",
                "\"evidence\":{\"unknown\":0,",
                1
            ))
            .is_err()
        );
    }

    #[test]
    fn held_descriptor_defeats_path_substitution_and_detects_byte_mutation() {
        let temp = Temp::new("held");
        let path = temp.0.join("input");
        fs::write(&path, b"original").expect("write original");
        let mut held = open_and_read_absolute(&path, 64).expect("held input");
        assert_eq!(authenticated_bytes(&held, 64).expect("bytes"), b"original");
        let renamed = temp.0.join("held-original");
        fs::rename(&path, &renamed).expect("rename original");
        fs::write(&path, b"replacement").expect("substitute path");
        assert_eq!(
            authenticated_bytes(&held, 64).expect("same held bytes"),
            b"original"
        );
        fs::write(&renamed, b"mutated!").expect("mutate held inode");
        assert_eq!(
            revalidate_held(&mut held)
                .expect_err("mutated held inode")
                .code,
            "EVIDENCE_MUTATION"
        );
    }

    #[test]
    fn held_authentication_leaves_read_only_evidence_unchanged() {
        let temp = Temp::new("immutable");
        let path = temp.0.join("evidence");
        fs::write(&path, b"immutable-v1-evidence").expect("write evidence");
        fs::set_permissions(&path, Permissions::from_mode(0o444)).expect("read-only");
        let before = fs::metadata(&path).expect("before metadata");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let mut held = open_relative_regular(&parent, Path::new("evidence"), 64).expect("open");
        authenticate_held(&mut held).expect("authenticate");
        revalidate_held(&mut held).expect("revalidate");
        let after = fs::metadata(&path).expect("after metadata");
        assert_eq!(before.dev(), after.dev());
        assert_eq!(before.ino(), after.ino());
        assert_eq!(before.len(), after.len());
        assert_eq!(before.mode(), after.mode());
        assert_eq!(fs::read(&path).expect("bytes"), b"immutable-v1-evidence");
    }

    #[test]
    fn small_evidence_semantic_checks_reuse_one_authenticated_buffer() {
        let temp = Temp::new("read-once");
        let path = temp.0.join("small.json");
        fs::write(&path, b"{\"ok\":true}").expect("small evidence");
        let held = open_and_read_absolute(&path, MAX_SMALL).expect("held small evidence");
        let mut files = BTreeMap::new();
        files.insert("small", held);
        let evidence = HeldEvidence { files };
        for _ in 0..5 {
            let bytes = evidence.bytes("small", MAX_SMALL).expect("same buffer");
            let parsed: serde_json::Value = serde_json::from_slice(bytes).expect("semantic parse");
            assert_eq!(parsed["ok"], true);
        }
        assert_eq!(
            evidence
                .file("small")
                .expect("small evidence")
                .descriptor_read_passes,
            1,
            "semantic validation must not reread the descriptor"
        );
    }

    #[test]
    fn resource_outcomes_and_failure_phases_have_exact_wire_values() {
        let outcomes = [
            (
                ResourceOutcome::QualificationPassedBeforePublication,
                "\"qualification-passed-before-publication\"",
            ),
            (ResourceOutcome::PreflightFailed, "\"preflight-failed\""),
            (ResourceOutcome::MeasurementFailed, "\"measurement-failed\""),
            (ResourceOutcome::PublicationFailed, "\"publication-failed\""),
        ];
        for (outcome, expected) in outcomes {
            assert_eq!(serde_json::to_string(&outcome).expect("outcome"), expected);
        }
        for (phase, expected) in [
            (FailurePhase::Preflight, "\"preflight\""),
            (FailurePhase::Measurement, "\"measurement\""),
            (FailurePhase::Publication, "\"publication\""),
        ] {
            assert_eq!(serde_json::to_string(&phase).expect("phase"), expected);
        }
    }

    fn populate_stage(stage: &QualificationStage) {
        stage
            .write_file("reuse-input.json", b"{}", 0o600)
            .expect("input");
        stage
            .write_file("command.json", b"{}", 0o600)
            .expect("command");
        stage
            .write_file("qualification-report.json", b"{}", 0o600)
            .expect("report");
        stage
            .write_file("qualification-resource.json", b"{}", 0o600)
            .expect("resource");
        stage
            .write_file("qualification.stderr.log", b"", 0o600)
            .expect("stderr");
        stage.mkdir("candidate", 0o700).expect("candidate");
        stage
            .write_file("candidate/pangopup-reference-benchmark", b"bin", 0o500)
            .expect("bin");
        stage
            .write_file(
                "candidate/pangopup-reference-benchmark.rs",
                b"source",
                0o600,
            )
            .expect("source");
    }

    fn held_test_executable(temp: &Temp) -> HeldFile {
        let path = temp.0.join(format!(
            "replacement-bin-{}",
            NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, b"bin").expect("replacement executable");
        open_and_read_absolute(&path, 64).expect("held executable")
    }

    static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(0);

    fn prepared_success_stage(temp: &Temp, label: &str) -> (QualificationStage, Report) {
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let stage = QualificationStage::create(&parent, &temp.0, label).expect("stage");
        populate_stage(&stage);
        let mut report = sample_report();
        report.identities.retained = RetainedIdentities {
            reuse_input: identity_bytes(b"{}"),
            command: identity_bytes(b"{}"),
            resource: identity_bytes(b"{}"),
            stderr: identity_bytes(b""),
            benchmark_executable: identity_bytes(b"bin"),
            harness_source: identity_bytes(b"source"),
        };
        stage
            .replace_file(
                "qualification-report.json",
                &canonical(&report, "report").expect("report"),
                0o600,
            )
            .expect("replace report");
        stage.seal_success().expect("seal success");
        validate_success_stage(&stage, &report).expect("valid success stage");
        (stage, report)
    }

    fn published_failure_stage(
        temp: &Temp,
        label: &str,
        error: Failure,
    ) -> (QualificationStage, PathBuf) {
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let mut stage = QualificationStage::create(&parent, &temp.0, label).expect("stage");
        let executable = held_test_executable(temp);
        let prior = ident("prior");
        try_seal_and_publish_failure(
            &mut stage,
            &parent,
            label,
            &prior.sha256,
            &error,
            FailureTiming {
                started: 1,
                started_at: Instant::now(),
                usage_before: usage().expect("usage"),
            },
            FailureIdentities {
                reuse: &identity_bytes(b"{}"),
                command: &identity_bytes(b"{}"),
                executable: &identity_bytes(b"bin"),
                harness: &identity_bytes(HARNESS_SOURCE),
            },
            b"{}",
            b"{}",
            &executable,
        )
        .expect("publish failure stage");
        let path = fs::read_dir(&temp.0)
            .expect("parent")
            .map(|entry| entry.expect("entry").path())
            .find(|path| {
                path.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| {
                        name.starts_with(&format!(".{label}.qualification-failed-"))
                    })
            })
            .expect("published failure root");
        validate_failure_stage(&stage).expect("valid failure stage");
        (stage, path)
    }

    fn rewrite_failure_receipt(path: &Path, update: impl FnOnce(&mut FailureReceipt)) {
        let receipt_path = path.join("failure.json");
        let bytes = fs::read(&receipt_path).expect("failure receipt");
        let mut receipt: FailureReceipt = parse_canonical(&bytes, "failure").expect("receipt");
        update(&mut receipt);
        fs::set_permissions(&receipt_path, Permissions::from_mode(0o600))
            .expect("receipt writable");
        fs::write(
            &receipt_path,
            canonical(&receipt, "failure").expect("canonical receipt"),
        )
        .expect("rewrite receipt");
        fs::set_permissions(&receipt_path, Permissions::from_mode(0o444)).expect("receipt sealed");
    }

    fn rewrite_success_report(stage: &QualificationStage, bytes: &[u8]) {
        let path = stage.path.join("qualification-report.json");
        fs::set_permissions(&path, Permissions::from_mode(0o600)).expect("report writable");
        fs::write(&path, bytes).expect("rewrite report");
        fs::set_permissions(&path, Permissions::from_mode(0o444)).expect("report sealed");
    }

    #[test]
    fn maximal_threshold_diagnostic_seals_in_the_failure_schema() {
        let temp = Temp::new("max-threshold-diagnostic");
        let pages = [
            31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
        ];
        let invalid = "x\nsecret-\u{2603}".repeat(65_536);
        let observed = ReferenceQualificationMeasurements {
            total_bases: u64::MAX,
            sequence_set_sha256: &invalid,
            extra_record_count: 680,
            extra_accessions_sha256: "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
            contexts_verified: u64::MAX,
            headline_p50_ns: 5_586,
            headline_p95_ns: 6_100,
            allocation_calls: 0,
            allocation_bytes: 0,
            open_peak_heap_bytes: 2_097_152,
            builder_peak_heap_bytes: 16_777_216,
            dense_bytes_read_during_open: 0,
            member_bytes: 773_124_288,
            unique_dense_pages: &pages,
            per_case_page_count_sum: 20,
        };
        let rejection =
            evaluate_reference_qualification(&observed).expect_err("maximal logical values");
        assert!(rejection.message.is_ascii());
        assert!(rejection.message.len() <= 256);
        assert!(!rejection.message.contains("secret"));
        let expected = rejection.message.clone();
        let (stage, path) = published_failure_stage(
            &temp,
            "max-threshold",
            Failure::measurement("THRESHOLD", rejection.message),
        );
        let receipt: FailureReceipt = parse_canonical(
            &fs::read(path.join("failure.json")).expect("failure receipt"),
            "failure",
        )
        .expect("closed failure receipt");
        assert_eq!(receipt.code, "THRESHOLD");
        assert_eq!(receipt.message, expected);
        assert_eq!(receipt.report_state, ReportState::Empty);
        validate_failure_stage(&stage).expect("maximal diagnostic failure stage");
    }

    #[test]
    fn success_publication_is_no_replace_and_has_exact_members_and_modes() {
        let temp = Temp::new("publish");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let mut blocked = QualificationStage::create(&parent, &temp.0, "blocked").expect("stage");
        populate_stage(&blocked);
        blocked.seal_success().expect("seal");
        fs::create_dir(temp.0.join("existing")).expect("existing");
        assert_eq!(
            blocked
                .publish_success(&parent, OsStr::new("existing"))
                .expect_err("existing destination")
                .code,
            "ALREADY_EXISTS"
        );
        assert!(rustix::fs::statat(&parent, &blocked.name, rustix::fs::AtFlags::empty()).is_ok());

        let mut stage = QualificationStage::create(&parent, &temp.0, "success").expect("stage");
        populate_stage(&stage);
        stage.seal_success().expect("seal");
        stage
            .publish_success(&parent, OsStr::new("success"))
            .expect("publish");
        let root = temp.0.join("success");
        let mut root_names: Vec<_> = fs::read_dir(&root)
            .expect("root")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        root_names.sort();
        assert_eq!(
            root_names,
            [
                "candidate",
                "command.json",
                "qualification-report.json",
                "qualification-resource.json",
                "qualification.stderr.log",
                "reuse-input.json",
            ]
            .map(OsString::from)
        );
        let mut candidate: Vec<_> = fs::read_dir(root.join("candidate"))
            .expect("candidate")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        candidate.sort();
        assert_eq!(
            candidate,
            [
                "pangopup-reference-benchmark",
                "pangopup-reference-benchmark.rs"
            ]
            .map(OsString::from)
        );
        assert_eq!(
            fs::metadata(root.join("candidate/pangopup-reference-benchmark"))
                .expect("executable")
                .mode()
                & 0o777,
            0o555
        );
        assert_eq!(
            fs::metadata(root.join("qualification-report.json"))
                .expect("report")
                .mode()
                & 0o777,
            0o444
        );
    }

    #[test]
    fn success_stage_validator_rejects_inventory_type_mode_and_every_identity_drift() {
        let temp = Temp::new("success-negatives");

        let (stage, report) = prepared_success_stage(&temp, "extra");
        fs::write(stage.path.join("extra"), b"extra").expect("extra member");
        assert_eq!(
            validate_success_stage(&stage, &report)
                .expect_err("extra member")
                .code,
            "STAGE_MEMBERS"
        );

        let (stage, report) = prepared_success_stage(&temp, "missing");
        fs::remove_file(stage.path.join("command.json")).expect("remove member");
        assert_eq!(
            validate_success_stage(&stage, &report)
                .expect_err("missing member")
                .code,
            "STAGE_MEMBERS"
        );

        let (stage, report) = prepared_success_stage(&temp, "wrong-type");
        let command = stage.path.join("command.json");
        fs::remove_file(&command).expect("remove command");
        fs::create_dir(&command).expect("wrong type");
        assert!(validate_success_stage(&stage, &report).is_err());

        let (stage, report) = prepared_success_stage(&temp, "wrong-mode");
        fs::set_permissions(
            stage.path.join("command.json"),
            Permissions::from_mode(0o644),
        )
        .expect("wrong mode");
        assert_eq!(
            validate_success_stage(&stage, &report)
                .expect_err("wrong mode")
                .code,
            "STAGE_MODE"
        );

        for index in 0..6 {
            let (stage, mut report) = prepared_success_stage(&temp, &format!("identity-{index}"));
            let wrong = ident(&format!("wrong-{index}"));
            match index {
                0 => report.identities.retained.reuse_input = wrong,
                1 => report.identities.retained.command = wrong,
                2 => report.identities.retained.resource = wrong,
                3 => report.identities.retained.stderr = wrong,
                4 => report.identities.retained.benchmark_executable = wrong,
                5 => report.identities.retained.harness_source = wrong,
                _ => unreachable!(),
            }
            assert_eq!(
                validate_success_stage(&stage, &report)
                    .expect_err("retained identity mismatch")
                    .code,
                "STAGE_IDENTITY"
            );
        }
    }

    #[test]
    fn success_publication_rejects_changed_truncated_and_noncanonical_report_bytes() {
        let temp = Temp::new("success-report-negatives");
        for (index, mutation) in [
            "changed",
            "truncated",
            "noncanonical",
            "duplicate",
            "unknown",
        ]
        .into_iter()
        .enumerate()
        {
            let parent = open_absolute_directory(&temp.0).expect("parent");
            let (mut stage, report) = prepared_success_stage(&temp, &format!("report-{index}"));
            let canonical_bytes = canonical(&report, "report").expect("canonical report");
            let bytes = match mutation {
                "changed" => {
                    let mut changed = sample_report();
                    changed.identities.retained = report.identities.retained.clone();
                    changed.logical.total_bases = 2;
                    canonical(&changed, "changed report").expect("changed report")
                }
                "truncated" => canonical_bytes[..canonical_bytes.len() - 1].to_vec(),
                "noncanonical" => serde_json::to_vec_pretty(&report).expect("pretty report"),
                "duplicate" => String::from_utf8(canonical_bytes.clone())
                    .expect("UTF-8 report")
                    .replacen("\"passed\":true", "\"passed\":true,\"passed\":true", 1)
                    .into_bytes(),
                "unknown" => String::from_utf8(canonical_bytes.clone())
                    .expect("UTF-8 report")
                    .replacen("{", "{\"unknown\":0,", 1)
                    .into_bytes(),
                _ => unreachable!(),
            };
            rewrite_success_report(&stage, &bytes);
            let output = OsString::from(format!("report-output-{index}"));
            assert!(
                validate_and_publish_success(&mut stage, &parent, &output, &report).is_err(),
                "{mutation} report must fail before publication"
            );
            assert!(
                !temp.0.join(&output).exists(),
                "{mutation} report must not publish a success root"
            );
        }
    }

    #[test]
    fn all_post_stage_failure_points_use_the_single_failure_funnel() {
        let temp = Temp::new("failure-funnel");
        for (index, (code, phase)) in [
            ("INITIAL_COPY", FailurePhase::Preflight),
            ("REVALIDATE", FailurePhase::Measurement),
            ("RESOURCE_SERIALIZE", FailurePhase::Measurement),
            ("REPORT_WRITE", FailurePhase::Publication),
            ("STAGE_VALIDATE", FailurePhase::Publication),
            ("SUCCESS_REPLACE", FailurePhase::Publication),
        ]
        .into_iter()
        .enumerate()
        {
            let (stage, path) = published_failure_stage(
                &temp,
                &format!("funnel-{index}"),
                Failure {
                    phase,
                    code,
                    message: "injected late failure".to_owned(),
                },
            );
            let receipt: FailureReceipt = parse_canonical(
                &fs::read(path.join("failure.json")).expect("failure receipt"),
                "failure",
            )
            .expect("closed failure receipt");
            assert_eq!(receipt.code, code);
            assert_eq!(receipt.phase, phase);
            validate_failure_stage(&stage).expect("sealed through common funnel");
        }
    }

    #[test]
    fn success_no_replace_error_is_resealed_as_publication_failure() {
        let temp = Temp::new("success-reseal");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let (mut stage, report) = prepared_success_stage(&temp, "success-reseal-stage");
        fs::create_dir(temp.0.join("occupied-success")).expect("occupied success");
        let publication = stage
            .publish_success(&parent, OsStr::new("occupied-success"))
            .expect_err("success no-replace");
        assert_eq!(publication.phase, FailurePhase::Publication);
        let executable = held_test_executable(&temp);
        let prior = ident("prior");
        seal_and_publish_failure(
            &mut stage,
            &parent,
            "success-reseal-stage",
            &prior.sha256,
            publication,
            FailureTiming {
                started: 1,
                started_at: Instant::now(),
                usage_before: usage().expect("usage"),
            },
            FailureIdentities {
                reuse: &report.identities.retained.reuse_input,
                command: &report.identities.retained.command,
                executable: &identity_bytes(b"bin"),
                harness: &identity_bytes(HARNESS_SOURCE),
            },
            b"{}",
            b"{}",
            &executable,
        )
        .expect("reseal publication failure");
        validate_failure_stage(&stage).expect("publication failure stage");
        let receipt = stage_member_bytes(&stage, "failure.json", MAX_SMALL).expect("receipt");
        let receipt: FailureReceipt = parse_canonical(&receipt, "failure").expect("closed receipt");
        assert_eq!(receipt.phase, FailurePhase::Publication);
        assert_eq!(receipt.report_state, ReportState::CompleteUnpublished);
    }

    #[test]
    fn failure_reseal_follows_held_stage_after_path_substitution() {
        let temp = Temp::new("failure-held-stage");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let (mut stage, report) = prepared_success_stage(&temp, "held-stage");
        let stale_path = stage.path.clone();
        let moved_path = temp.0.join("concurrently-moved-stage");
        fs::rename(&stale_path, &moved_path).expect("rename held stage");
        fs::create_dir(&stale_path).expect("substitute stale path");
        fs::write(stale_path.join("sentinel"), b"do not touch").expect("substitute sentinel");
        let executable = held_test_executable(&temp);
        let prior = ident("prior");
        seal_and_publish_failure(
            &mut stage,
            &parent,
            "held-stage",
            &prior.sha256,
            Failure::publication("INJECTED", "path substitution injection"),
            FailureTiming {
                started: 1,
                started_at: Instant::now(),
                usage_before: usage().expect("usage"),
            },
            FailureIdentities {
                reuse: &report.identities.retained.reuse_input,
                command: &report.identities.retained.command,
                executable: &identity_bytes(b"bin"),
                harness: &identity_bytes(HARNESS_SOURCE),
            },
            b"{}",
            b"{}",
            &executable,
        )
        .expect("publish held failure stage");
        assert!(stage.path.exists(), "reported published path must exist");
        assert!(
            stage
                .path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.contains("qualification-failed")),
            "reported path must be the current published failure root"
        );
        assert!(
            !moved_path.exists(),
            "held stage must move to its failure name"
        );
        assert_eq!(
            fs::read(stale_path.join("sentinel")).expect("substitute sentinel"),
            b"do not touch"
        );
        validate_failure_stage(&stage).expect("descriptor-relative failure stage");
    }

    #[test]
    fn failure_publication_exhaustion_reports_exact_preserved_stage_path() {
        let temp = Temp::new("failure-preserved-path");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let contract = "exhausted";
        let mut stage = QualificationStage::create(&parent, &temp.0, contract).expect("stage");
        let stale_path = stage.path.clone();
        let moved_path = temp.0.join("concurrently-moved-preserved-stage");
        fs::rename(&stale_path, &moved_path).expect("rename held stage");
        fs::create_dir(&stale_path).expect("substitute stale path");
        fs::write(stale_path.join("sentinel"), b"do not touch").expect("substitute sentinel");
        for counter in 0..1024_u64 {
            fs::create_dir(temp.0.join(format!(
                ".{contract}.qualification-failed-{}-{counter}",
                std::process::id()
            )))
            .expect("occupy failure name");
        }
        let executable = held_test_executable(&temp);
        let prior = ident("prior");
        let error = seal_and_publish_failure(
            &mut stage,
            &parent,
            contract,
            &prior.sha256,
            Failure::measurement("INJECTED", "failure publication injection"),
            FailureTiming {
                started: 1,
                started_at: Instant::now(),
                usage_before: usage().expect("usage"),
            },
            FailureIdentities {
                reuse: &identity_bytes(b"{}"),
                command: &identity_bytes(b"{}"),
                executable: &identity_bytes(b"bin"),
                harness: &identity_bytes(HARNESS_SOURCE),
            },
            b"{}",
            b"{}",
            &executable,
        )
        .expect_err("failure publication exhaustion");
        assert_eq!(error.code, "STAGE_PRESERVED");
        assert_eq!(stage.path, moved_path);
        assert!(
            error
                .message
                .contains(moved_path.to_str().expect("UTF-8 path"))
        );
        assert!(stage.path.exists(), "private stage must remain preserved");
        assert_eq!(
            fs::read(stale_path.join("sentinel")).expect("substitute sentinel"),
            b"do not touch"
        );
    }

    #[test]
    fn failure_publication_preserves_closed_evidence_without_success_root() {
        let temp = Temp::new("failure");
        let parent = open_absolute_directory(&temp.0).expect("parent");
        let mut stage = QualificationStage::create(&parent, &temp.0, "contract").expect("stage");
        populate_stage(&stage);
        let i = ident("i");
        let executable_path = temp.0.join("replacement-bin");
        fs::write(&executable_path, b"bin").expect("replacement executable");
        let executable = open_and_read_absolute(&executable_path, 64).expect("held executable");
        seal_and_publish_failure(
            &mut stage,
            &parent,
            "contract",
            &i.sha256,
            Failure::measurement("TEST", "miniature failure"),
            FailureTiming {
                started: 1,
                started_at: Instant::now(),
                usage_before: usage().expect("usage"),
            },
            FailureIdentities {
                reuse: &identity_bytes(b"{}"),
                command: &identity_bytes(b"{}"),
                executable: &identity_bytes(b"bin"),
                harness: &identity_bytes(HARNESS_SOURCE),
            },
            b"{}",
            b"{}",
            &executable,
        )
        .expect("publish failure");
        assert!(!temp.0.join("contract").exists());
        let failure_root = fs::read_dir(&temp.0)
            .expect("parent")
            .map(|entry| entry.expect("entry").path())
            .find(|path| {
                path.file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.contains("qualification-failed"))
            })
            .expect("failure root");
        let receipt_bytes = fs::read(failure_root.join("failure.json")).expect("receipt");
        let receipt: FailureReceipt =
            parse_canonical(&receipt_bytes, "failure").expect("closed receipt");
        assert_eq!(receipt.phase, FailurePhase::Measurement);
        assert_eq!(receipt.report_state, ReportState::Empty);
        assert_eq!(
            receipt.evidence.stderr,
            identity_bytes(b"reference qualification failed: TEST: miniature failure\n")
        );
    }

    #[test]
    fn failure_stage_validator_rejects_inventory_type_mode_and_every_evidence_drift() {
        let temp = Temp::new("failure-negatives");

        let (stage, path) =
            published_failure_stage(&temp, "failure-extra", Failure::measurement("TEST", "test"));
        fs::write(path.join("extra"), b"extra").expect("extra");
        assert_eq!(
            validate_failure_stage(&stage)
                .expect_err("extra failure member")
                .code,
            "STAGE_MEMBERS"
        );

        let (stage, path) = published_failure_stage(
            &temp,
            "failure-missing",
            Failure::measurement("TEST", "test"),
        );
        fs::remove_file(path.join("command.json")).expect("missing");
        assert_eq!(
            validate_failure_stage(&stage)
                .expect_err("missing failure member")
                .code,
            "STAGE_MEMBERS"
        );

        let (stage, path) =
            published_failure_stage(&temp, "failure-type", Failure::measurement("TEST", "test"));
        fs::remove_file(path.join("command.json")).expect("remove");
        fs::create_dir(path.join("command.json")).expect("wrong type");
        assert!(validate_failure_stage(&stage).is_err());

        let (stage, path) =
            published_failure_stage(&temp, "failure-mode", Failure::measurement("TEST", "test"));
        fs::set_permissions(path.join("command.json"), Permissions::from_mode(0o644))
            .expect("wrong mode");
        assert_eq!(
            validate_failure_stage(&stage)
                .expect_err("wrong failure mode")
                .code,
            "STAGE_MODE"
        );

        for index in 0..7 {
            let (stage, path) = published_failure_stage(
                &temp,
                &format!("failure-identity-{index}"),
                Failure::measurement("TEST", "test"),
            );
            rewrite_failure_receipt(&path, |receipt| {
                let wrong = ident(&format!("wrong-{index}"));
                match index {
                    0 => receipt.evidence.reuse_input = wrong,
                    1 => receipt.evidence.command = wrong,
                    2 => receipt.evidence.resource = wrong,
                    3 => receipt.evidence.stderr = wrong,
                    4 => receipt.evidence.report = wrong,
                    5 => receipt.evidence.benchmark_executable = wrong,
                    6 => receipt.evidence.harness_source = wrong,
                    _ => unreachable!(),
                }
            });
            assert_eq!(
                validate_failure_stage(&stage)
                    .expect_err("failure evidence identity mismatch")
                    .code,
                "STAGE_IDENTITY"
            );
        }
    }

    #[test]
    fn compiled_inventory_and_harness_are_bound_to_this_candidate() {
        validate_sha(SOURCE_INVENTORY).expect("inventory digest");
        assert_eq!(
            identity_bytes(HARNESS_SOURCE).bytes,
            HARNESS_SOURCE.len() as u64
        );
        assert_ne!(SOURCE_INVENTORY, PRIOR_BUILDER_SOURCE);
    }
}
