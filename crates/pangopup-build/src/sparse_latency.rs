//! Maintainer-only complete fixed-v1 versus sparse-candidate latency measurement.

use crate::CommandError;
use pangopup_core::{EnsemblGeneId, Grch38Snv};
use pangopup_index::{IndexReader, sparse_reader::SparseIndexReader};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    hint::black_box,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

pub const FIXED_BYTES: u64 = 15_033_158_255;
pub const FIXED_SHA256: &str = "6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27";
pub const CANDIDATE_BYTES: u64 = 2_035_371_437;
pub const CANDIDATE_SHA256: &str =
    "354343dc1a9f6558e46693e2481b5181461be2115cd92e029ffd4d4abef4a01e";
pub const QUERY_BYTES: u64 = 6_938;
pub const QUERY_SHA256: &str = "e1d8ffb9bf0077d7b48acf49bb436d7e7453ed3abb3497269b87e09c722aa0bd";
pub const SELECTION_BYTES: u64 = 6_569;
pub const SELECTION_SHA256: &str =
    "056c974c64bfc78b42640b58cb548ec95d7cfe0fca8edcf2f5c19f2a35985a70";
const WARMUPS: usize = 20;
const SAMPLES: usize = 20;

#[derive(Clone, Debug)]
pub struct SparseLatencyArguments {
    pub fixed: PathBuf,
    pub candidate: PathBuf,
    pub queries: PathBuf,
    pub selection: PathBuf,
    pub output: PathBuf,
    pub command_commit: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SparseLatencyOutcome {
    pub status: &'static str,
    pub report_schema: &'static str,
    pub report_bytes: u64,
    pub report_sha256: String,
    pub all_latency_gates_passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct MemberIdentity {
    bytes: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ExecutableIdentity {
    bytes: u64,
    sha256: String,
    builder_source_sha256: String,
    compiled_git_commit: String,
    compiled_git_clean: bool,
    rustc_version_verbose: String,
    target: String,
    build_profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Environment {
    kernel: String,
    ubuntu_release: String,
    cpu: String,
    process_affinity: String,
    cpu_governor: String,
    energy_performance_preference: String,
    memory_bytes: u64,
    storage_model: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Inputs {
    fixed: MemberIdentity,
    candidate: MemberIdentity,
    query_manifest: MemberIdentity,
    selected_genes: MemberIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ExactRatio {
    candidate_p50_ns: u64,
    fixed_p50_ns: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct WorkloadReport {
    requests: usize,
    fixed_samples_ns: Vec<u64>,
    candidate_samples_ns: Vec<u64>,
    fixed_p50_ns: u64,
    candidate_p50_ns: u64,
    candidate_to_fixed_ratio: ExactRatio,
    ten_times_passed: bool,
    absolute_limit_ns: u64,
    absolute_limit_passed: bool,
    passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Report {
    schema: &'static str,
    method: &'static str,
    command_commit: String,
    release_executable: ExecutableIdentity,
    environment: Environment,
    inputs: Inputs,
    warmups_per_reader_per_workload: usize,
    retained_samples_per_reader_per_workload: usize,
    workloads: Vec<WorkloadReport>,
    all_latency_gates_passed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Query {
    snv: Grch38Snv,
    gene: EnsemblGeneId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReaderOrder {
    FixedThenCandidate,
    CandidateThenFixed,
}

/// Run the closed reference-host measurement and publish one canonical report.
pub fn measure_sparse_latency(
    arguments: &SparseLatencyArguments,
) -> Result<SparseLatencyOutcome, CommandError> {
    if arguments.output.exists() {
        return Err(fail("LATENCY_OUTPUT", "report output already exists"));
    }
    let command_commit = validate_git_checkout(&arguments.command_commit)?;
    let executable = executable_identity()?;
    if executable.build_profile != "release" {
        return Err(fail(
            "LATENCY_BUILD",
            "benchmark executable is not a release build",
        ));
    }
    if executable.target != "x86_64-unknown-linux-gnu" {
        return Err(fail(
            "LATENCY_BUILD",
            "benchmark executable has the wrong target",
        ));
    }
    let environment = inspect_environment(&arguments.fixed, &arguments.candidate)?;

    let (fixed_file, fixed_identity) =
        open_exact_regular(&arguments.fixed, "fixed member", FIXED_BYTES, FIXED_SHA256)?;
    let (candidate_file, candidate_identity) = open_exact_regular(
        &arguments.candidate,
        "candidate member",
        CANDIDATE_BYTES,
        CANDIDATE_SHA256,
    )?;
    let (query_file, query_identity) = open_exact_regular(
        &arguments.queries,
        "query manifest",
        QUERY_BYTES,
        QUERY_SHA256,
    )?;
    let (_selection_file, selection_identity) = open_exact_regular(
        &arguments.selection,
        "selected-gene manifest",
        SELECTION_BYTES,
        SELECTION_SHA256,
    )?;
    let query_bytes = read_held(&query_file, QUERY_BYTES, "query manifest")?;
    let queries = parse_queries(&query_bytes)?;

    let fixed = IndexReader::open_file(&fixed_file)
        .map_err(|error| fail("LATENCY_FIXED", format!("fixed member rejected: {error}")))?;
    let candidate = SparseIndexReader::open_file(&candidate_file).map_err(|error| {
        fail(
            "LATENCY_CANDIDATE",
            format!("candidate member rejected: {error}"),
        )
    })?;
    if fixed.file_len() != FIXED_BYTES || candidate.file_len() != CANDIDATE_BYTES {
        return Err(fail(
            "LATENCY_IDENTITY",
            "reader length disagrees with member identity",
        ));
    }

    let expected_totals = require_equal_answers(&fixed, &candidate, &queries)?;
    let mut workloads = Vec::new();
    for (request_count, absolute_limit_ns) in [(1, 2_100), (10, 19_640), (100, 195_880)] {
        workloads.push(measure_workload(
            &fixed,
            &candidate,
            &queries[..request_count],
            expected_totals[request_count - 1],
            absolute_limit_ns,
        )?);
    }
    let all_latency_gates_passed = workloads.iter().all(|workload| workload.passed);
    let report = Report {
        schema: "pangopup.sparse-index-latency.v1",
        method: "complete-members;warm-one-open;alternating-reader-order;20-warmups;20-retained;nearest-rank-p50",
        command_commit,
        release_executable: executable,
        environment,
        inputs: Inputs {
            fixed: fixed_identity,
            candidate: candidate_identity,
            query_manifest: query_identity,
            selected_genes: selection_identity,
        },
        warmups_per_reader_per_workload: WARMUPS,
        retained_samples_per_reader_per_workload: SAMPLES,
        workloads,
        all_latency_gates_passed,
    };
    let bytes = serde_jcs::to_vec(&report)
        .map_err(|_| fail("LATENCY_REPORT", "canonical report encoding failed"))?;
    publish_new(&arguments.output, &bytes)?;
    Ok(SparseLatencyOutcome {
        status: "measured",
        report_schema: report.schema,
        report_bytes: u64::try_from(bytes.len())
            .map_err(|_| fail("LATENCY_REPORT", "report length overflow"))?,
        report_sha256: hex_digest(&bytes),
        all_latency_gates_passed,
    })
}

fn require_equal_answers(
    fixed: &IndexReader,
    candidate: &SparseIndexReader,
    queries: &[Query],
) -> Result<Vec<(usize, usize)>, CommandError> {
    let mut cumulative_records = 0_usize;
    let mut cumulative_ambiguities = 0_usize;
    let mut totals = Vec::with_capacity(queries.len());
    for (index, query) in queries.iter().enumerate() {
        let fixed_answer = fixed
            .lookup_parts(query.snv, Some(query.gene))
            .map_err(|error| {
                fail(
                    "LATENCY_FIXED",
                    format!("query {} failed: {error}", index + 1),
                )
            })?;
        let candidate_answer = candidate
            .lookup_parts(query.snv, Some(query.gene))
            .map_err(|error| {
                fail(
                    "LATENCY_CANDIDATE",
                    format!("query {} failed: {error}", index + 1),
                )
            })?;
        require_matching_answer(&fixed_answer, &candidate_answer, index + 1)?;
        cumulative_records = cumulative_records
            .checked_add(fixed_answer.0.len())
            .ok_or_else(|| fail("LATENCY_OVERFLOW", "record total overflow"))?;
        cumulative_ambiguities = cumulative_ambiguities
            .checked_add(fixed_answer.1.len())
            .ok_or_else(|| fail("LATENCY_OVERFLOW", "ambiguity total overflow"))?;
        totals.push((cumulative_records, cumulative_ambiguities));
    }
    Ok(totals)
}

fn require_matching_answer<T: Eq>(
    fixed: &T,
    candidate: &T,
    query: usize,
) -> Result<(), CommandError> {
    if fixed == candidate {
        Ok(())
    } else {
        Err(fail(
            "LATENCY_MISMATCH",
            format!("readers disagree on query {query}"),
        ))
    }
}

fn measure_workload(
    fixed: &IndexReader,
    candidate: &SparseIndexReader,
    queries: &[Query],
    expected: (usize, usize),
    absolute_limit_ns: u64,
) -> Result<WorkloadReport, CommandError> {
    for sample in 0..WARMUPS {
        run_pair(sample, fixed, candidate, queries, expected)?;
    }
    let mut fixed_samples = Vec::with_capacity(SAMPLES);
    let mut candidate_samples = Vec::with_capacity(SAMPLES);
    for sample in 0..SAMPLES {
        let (fixed_ns, candidate_ns) = run_pair(sample, fixed, candidate, queries, expected)?;
        fixed_samples.push(fixed_ns);
        candidate_samples.push(candidate_ns);
    }
    workload_report(
        queries.len(),
        fixed_samples,
        candidate_samples,
        absolute_limit_ns,
    )
}

fn run_pair(
    sample: usize,
    fixed: &IndexReader,
    candidate: &SparseIndexReader,
    queries: &[Query],
    expected: (usize, usize),
) -> Result<(u64, u64), CommandError> {
    run_pair_with(
        sample,
        || time_fixed(fixed, queries, expected),
        || time_candidate(candidate, queries, expected),
    )
}

fn run_pair_with<T>(
    sample: usize,
    mut fixed: impl FnMut() -> Result<T, CommandError>,
    mut candidate: impl FnMut() -> Result<T, CommandError>,
) -> Result<(T, T), CommandError> {
    match reader_order(sample) {
        ReaderOrder::FixedThenCandidate => {
            let fixed_value = fixed()?;
            let candidate_value = candidate()?;
            Ok((fixed_value, candidate_value))
        }
        ReaderOrder::CandidateThenFixed => {
            let candidate_value = candidate()?;
            let fixed_value = fixed()?;
            Ok((fixed_value, candidate_value))
        }
    }
}

fn reader_order(sample: usize) -> ReaderOrder {
    if sample.is_multiple_of(2) {
        ReaderOrder::FixedThenCandidate
    } else {
        ReaderOrder::CandidateThenFixed
    }
}

fn time_fixed(
    reader: &IndexReader,
    queries: &[Query],
    expected: (usize, usize),
) -> Result<u64, CommandError> {
    time_reader(queries, expected, |query| {
        reader
            .lookup_parts(query.snv, Some(query.gene))
            .map_err(|error| fail("LATENCY_FIXED", format!("timed lookup failed: {error}")))
    })
}

fn time_candidate(
    reader: &SparseIndexReader,
    queries: &[Query],
    expected: (usize, usize),
) -> Result<u64, CommandError> {
    time_reader(queries, expected, |query| {
        reader
            .lookup_parts(query.snv, Some(query.gene))
            .map_err(|error| fail("LATENCY_CANDIDATE", format!("timed lookup failed: {error}")))
    })
}

fn time_reader<T, U>(
    queries: &[Query],
    expected: (usize, usize),
    mut lookup: impl FnMut(&Query) -> Result<(Vec<T>, Vec<U>), CommandError>,
) -> Result<u64, CommandError> {
    let start = Instant::now();
    let mut records = 0_usize;
    let mut ambiguities = 0_usize;
    for query in queries {
        let answer = lookup(query)?;
        black_box(&answer);
        records = records
            .checked_add(answer.0.len())
            .ok_or_else(|| fail("LATENCY_OVERFLOW", "timed record total overflow"))?;
        ambiguities = ambiguities
            .checked_add(answer.1.len())
            .ok_or_else(|| fail("LATENCY_OVERFLOW", "timed ambiguity total overflow"))?;
        drop(answer);
    }
    let elapsed = start.elapsed();
    if (records, ambiguities) != expected {
        return Err(fail(
            "LATENCY_MISMATCH",
            "timed lookup totals differ from the preflight totals",
        ));
    }
    u64::try_from(elapsed.as_nanos())
        .map_err(|_| fail("LATENCY_OVERFLOW", "nanosecond duration overflow"))
}

fn workload_report(
    requests: usize,
    fixed_samples_ns: Vec<u64>,
    candidate_samples_ns: Vec<u64>,
    absolute_limit_ns: u64,
) -> Result<WorkloadReport, CommandError> {
    if !matches!(requests, 1 | 10 | 100)
        || fixed_samples_ns.len() != SAMPLES
        || candidate_samples_ns.len() != SAMPLES
        || absolute_limit_ns == 0
    {
        return Err(fail("LATENCY_SAMPLES", "invalid workload or sample count"));
    }
    let fixed_p50_ns = nearest_rank_p50(&fixed_samples_ns)?;
    let candidate_p50_ns = nearest_rank_p50(&candidate_samples_ns)?;
    if fixed_p50_ns == 0 {
        return Err(fail("LATENCY_SAMPLES", "fixed p50 must be nonzero"));
    }
    let ten_times_limit = fixed_p50_ns
        .checked_mul(10)
        .ok_or_else(|| fail("LATENCY_OVERFLOW", "ten-times gate overflow"))?;
    let ten_times_passed = candidate_p50_ns <= ten_times_limit;
    let absolute_limit_passed = candidate_p50_ns <= absolute_limit_ns;
    Ok(WorkloadReport {
        requests,
        fixed_samples_ns,
        candidate_samples_ns,
        fixed_p50_ns,
        candidate_p50_ns,
        candidate_to_fixed_ratio: ExactRatio {
            candidate_p50_ns,
            fixed_p50_ns,
        },
        ten_times_passed,
        absolute_limit_ns,
        absolute_limit_passed,
        passed: ten_times_passed && absolute_limit_passed,
    })
}

fn nearest_rank_p50(samples: &[u64]) -> Result<u64, CommandError> {
    if samples.len() != SAMPLES {
        return Err(fail("LATENCY_SAMPLES", "p50 requires exactly 20 samples"));
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    Ok(sorted[samples.len().div_ceil(2) - 1])
}

fn parse_queries(bytes: &[u8]) -> Result<Vec<Query>, CommandError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| fail("LATENCY_MANIFEST", "query manifest is not UTF-8"))?;
    if text.contains('\r') || !text.ends_with('\n') {
        return Err(fail(
            "LATENCY_MANIFEST",
            "query manifest must use LF lines and end with LF",
        ));
    }
    let mut lines = text.lines();
    if lines.next() != Some("ordinal\tcontig\tposition\tref\talt\tgene\tworkload") {
        return Err(fail("LATENCY_MANIFEST", "query manifest header differs"));
    }
    let mut queries = Vec::with_capacity(100);
    let mut seen = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 7 {
            return Err(fail("LATENCY_MANIFEST", "query row column count differs"));
        }
        let expected_ordinal = index + 1;
        if columns[0].parse::<usize>().ok() != Some(expected_ordinal) {
            return Err(fail(
                "LATENCY_MANIFEST",
                "query ordinals are not strictly ordered from one",
            ));
        }
        if columns[6] != "primary-distinct-gene-filtered" {
            return Err(fail("LATENCY_MANIFEST", "query workload differs"));
        }
        let contig = columns[1]
            .parse()
            .map_err(|_| fail("LATENCY_MANIFEST", "query contig is invalid"))?;
        let position = columns[2]
            .parse::<u32>()
            .ok()
            .and_then(|value| pangopup_core::GenomicPosition::new(value).ok())
            .ok_or_else(|| fail("LATENCY_MANIFEST", "query position is invalid"))?;
        let reference = pangopup_core::DnaBase::parse(columns[3])
            .map_err(|_| fail("LATENCY_MANIFEST", "query reference is invalid"))?;
        let alternate = pangopup_core::DnaBase::parse(columns[4])
            .map_err(|_| fail("LATENCY_MANIFEST", "query alternate is invalid"))?;
        let gene = columns[5]
            .parse()
            .map_err(|_| fail("LATENCY_MANIFEST", "query gene is invalid"))?;
        let snv = Grch38Snv::new(contig, position, reference, alternate)
            .map_err(|_| fail("LATENCY_MANIFEST", "query SNV is invalid"))?;
        if !seen.insert((contig, position, reference, alternate, gene)) {
            return Err(fail(
                "LATENCY_MANIFEST",
                "query manifest contains a duplicate",
            ));
        }
        queries.push(Query { snv, gene });
    }
    if queries.len() != 100 {
        return Err(fail(
            "LATENCY_MANIFEST",
            "query manifest must contain exactly 100 requests",
        ));
    }
    Ok(queries)
}

fn open_exact_regular(
    path: &Path,
    label: &str,
    expected_bytes: u64,
    expected_sha256: &str,
) -> Result<(File, MemberIdentity), CommandError> {
    let file = File::open(path)
        .map_err(|error| fail("LATENCY_INPUT", format!("open {label}: {error}")))?;
    let metadata = file
        .metadata()
        .map_err(|error| fail("LATENCY_INPUT", format!("inspect {label}: {error}")))?;
    if !metadata.file_type().is_file() || metadata.len() != expected_bytes {
        return Err(fail(
            "LATENCY_IDENTITY",
            format!("{label} is not the required regular file and length"),
        ));
    }
    let actual_sha256 = hash_file(&file, expected_bytes, label)?;
    if actual_sha256 != expected_sha256 {
        return Err(fail("LATENCY_IDENTITY", format!("{label} SHA-256 differs")));
    }
    Ok((
        file,
        MemberIdentity {
            bytes: expected_bytes,
            sha256: actual_sha256,
        },
    ))
}

fn hash_file(file: &File, expected_bytes: u64, label: &str) -> Result<String, CommandError> {
    let mut reader = file
        .try_clone()
        .map_err(|error| fail("LATENCY_INPUT", format!("clone {label}: {error}")))?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| fail("LATENCY_INPUT", format!("rewind {label}: {error}")))?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| fail("LATENCY_INPUT", format!("read {label}: {error}")))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| fail("LATENCY_OVERFLOW", "input byte count overflow"))?;
        if total > expected_bytes {
            return Err(fail(
                "LATENCY_IDENTITY",
                format!("{label} grew while hashing"),
            ));
        }
        digest.update(&buffer[..count]);
    }
    if total != expected_bytes {
        return Err(fail(
            "LATENCY_IDENTITY",
            format!("{label} changed while hashing"),
        ));
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn read_held(file: &File, expected_bytes: u64, label: &str) -> Result<Vec<u8>, CommandError> {
    let mut file = file
        .try_clone()
        .map_err(|error| fail("LATENCY_INPUT", format!("clone {label}: {error}")))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| fail("LATENCY_INPUT", format!("rewind {label}: {error}")))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| fail("LATENCY_INPUT", format!("read {label}: {error}")))?;
    if bytes.len() as u64 != expected_bytes {
        return Err(fail(
            "LATENCY_IDENTITY",
            format!("{label} changed after hashing"),
        ));
    }
    Ok(bytes)
}

fn executable_identity() -> Result<ExecutableIdentity, CommandError> {
    let path = std::env::current_exe()
        .map_err(|error| fail("LATENCY_BUILD", format!("resolve executable: {error}")))?;
    let file = File::open(&path)
        .map_err(|error| fail("LATENCY_BUILD", format!("open executable: {error}")))?;
    let bytes = file
        .metadata()
        .map_err(|error| fail("LATENCY_BUILD", format!("inspect executable: {error}")))?
        .len();
    let rustc_version_verbose =
        command_output("rustc", &["--version", "--verbose"], "rustc version")?;
    let first = rustc_version_verbose.lines().next().unwrap_or_default();
    if first != env!("PANGOPUP_RUSTC_VERSION") {
        return Err(fail(
            "LATENCY_BUILD",
            "runtime rustc differs from build rustc",
        ));
    }
    let builder_source_sha256 = env!("PANGOPUP_BUILDER_SOURCE_SHA256");
    if builder_source_sha256.len() != 64
        || !builder_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(fail(
            "LATENCY_BUILD",
            "builder source fingerprint is unavailable or invalid",
        ));
    }
    let compiled_git_commit = env!("PANGOPUP_GIT_COMMIT");
    let compiled_git_clean = match env!("PANGOPUP_GIT_CLEAN") {
        "true" => true,
        "false" => false,
        _ => {
            return Err(fail(
                "LATENCY_BUILD",
                "compiled Git cleanliness is unavailable",
            ));
        }
    };
    if !compiled_git_clean {
        return Err(fail(
            "LATENCY_BUILD",
            "release executable was compiled from a dirty checkout",
        ));
    }
    Ok(ExecutableIdentity {
        bytes,
        sha256: hash_file(&file, bytes, "release executable")?,
        builder_source_sha256: builder_source_sha256.to_owned(),
        compiled_git_commit: compiled_git_commit.to_owned(),
        compiled_git_clean,
        rustc_version_verbose,
        target: env!("PANGOPUP_TARGET").to_owned(),
        build_profile: env!("PANGOPUP_BUILD_PROFILE").to_owned(),
    })
}

fn validate_git_checkout(expected: &str) -> Result<String, CommandError> {
    if expected.len() != 40
        || !expected
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(fail(
            "LATENCY_GIT",
            "command commit must be 40 lowercase hexadecimal characters",
        ));
    }
    let head = command_output("git", &["rev-parse", "HEAD"], "git HEAD")?;
    validate_commit_binding(
        expected,
        &head,
        env!("PANGOPUP_GIT_COMMIT"),
        env!("PANGOPUP_GIT_CLEAN"),
    )?;
    if !command_output(
        "git",
        &["status", "--porcelain", "--untracked-files=all"],
        "git status",
    )?
    .is_empty()
    {
        return Err(fail("LATENCY_GIT", "checkout is not clean"));
    }
    let remotes = command_output(
        "git",
        &["branch", "-r", "--contains", expected],
        "git remote containment",
    )?;
    if remotes.lines().all(|line| line.trim().is_empty()) {
        return Err(fail(
            "LATENCY_GIT",
            "command commit is not present on a remote branch",
        ));
    }
    Ok(head)
}

fn validate_commit_binding(
    expected: &str,
    checkout: &str,
    executable: &str,
    compiled_clean: &str,
) -> Result<(), CommandError> {
    if checkout != expected {
        return Err(fail(
            "LATENCY_GIT",
            "checkout HEAD differs from command commit",
        ));
    }
    if executable != expected {
        return Err(fail(
            "LATENCY_GIT",
            "release executable was built from a different commit",
        ));
    }
    if compiled_clean != "true" {
        return Err(fail(
            "LATENCY_GIT",
            "release executable was compiled without clean Git provenance",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn inspect_environment(fixed: &Path, candidate: &Path) -> Result<Environment, CommandError> {
    let os_release = fs::read_to_string("/etc/os-release")
        .map_err(|error| fail("LATENCY_HOST", format!("read OS release: {error}")))?;
    let ubuntu_release = parse_ubuntu_release(&os_release)?;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")
        .map_err(|error| fail("LATENCY_HOST", format!("read CPU information: {error}")))?;
    let models: BTreeSet<_> = cpuinfo
        .lines()
        .filter_map(|line| line.strip_prefix("model name\t:").map(str::trim))
        .collect();
    if models.len() != 1 {
        return Err(fail("LATENCY_HOST", "CPU model is missing or inconsistent"));
    }
    let cpu = (*models.first().expect("one model")).to_owned();
    if !cpu.contains("AMD Ryzen 7 5825U") {
        return Err(fail(
            "LATENCY_HOST",
            "reference CPU must be AMD Ryzen 7 5825U",
        ));
    }
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| fail("LATENCY_HOST", format!("read process status: {error}")))?;
    let affinity = status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:\t"))
        .ok_or_else(|| fail("LATENCY_HOST", "process affinity is missing"))?;
    if parse_cpu_list(affinity)? != (0_u16..16).collect::<Vec<_>>() {
        return Err(fail(
            "LATENCY_HOST",
            "benchmark needs all 16 default allowed CPUs",
        ));
    }
    let memory_bytes = parse_memory_bytes(
        &fs::read_to_string("/proc/meminfo")
            .map_err(|error| fail("LATENCY_HOST", format!("read memory information: {error}")))?,
    )?;
    let cpu_governor = one_policy_value("scaling_governor")?;
    let energy_performance_preference = one_policy_value("energy_performance_preference")?;
    let fixed_storage = storage_model(fixed)?;
    let candidate_storage = storage_model(candidate)?;
    if fixed_storage != "CT1000P3PSSD8" || candidate_storage != fixed_storage {
        return Err(fail(
            "LATENCY_HOST",
            "both members must reside on the recorded Crucial CT1000P3PSSD8 storage",
        ));
    }
    Ok(Environment {
        kernel: command_output("uname", &["-srvm"], "kernel")?,
        ubuntu_release,
        cpu,
        process_affinity: affinity.to_owned(),
        cpu_governor,
        energy_performance_preference,
        memory_bytes,
        storage_model: fixed_storage,
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_ubuntu_release(text: &str) -> Result<String, CommandError> {
    let mut id = None;
    let mut version = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = Some(value.trim_matches('"'));
        }
        if let Some(value) = line.strip_prefix("VERSION_ID=") {
            version = Some(value.trim_matches('"'));
        }
    }
    if id != Some("ubuntu") || version != Some("24.04") {
        return Err(fail("LATENCY_HOST", "reference host must run Ubuntu 24.04"));
    }
    Ok("24.04".to_owned())
}

#[cfg(not(target_os = "linux"))]
fn inspect_environment(_fixed: &Path, _candidate: &Path) -> Result<Environment, CommandError> {
    Err(fail(
        "LATENCY_HOST",
        "authoritative latency measurement requires the retained Linux host",
    ))
}

#[cfg(target_os = "linux")]
fn one_policy_value(name: &str) -> Result<String, CommandError> {
    let root = Path::new("/sys/devices/system/cpu/cpufreq");
    let entries = fs::read_dir(root).map_err(|error| {
        fail(
            "LATENCY_HOST",
            format!("read CPU frequency policies: {error}"),
        )
    })?;
    let mut values = BTreeSet::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| fail("LATENCY_HOST", format!("read CPU policy: {error}")))?;
        if entry.file_name().to_string_lossy().starts_with("policy") {
            let value = fs::read_to_string(entry.path().join(name)).map_err(|error| {
                fail("LATENCY_HOST", format!("read CPU policy {name}: {error}"))
            })?;
            values.insert(value.trim().to_owned());
        }
    }
    if values.len() != 1 || values.contains("") {
        return Err(fail(
            "LATENCY_HOST",
            format!("CPU policy {name} is missing or inconsistent"),
        ));
    }
    Ok(values.into_iter().next().expect("one policy value"))
}

#[cfg(target_os = "linux")]
fn storage_model(path: &Path) -> Result<String, CommandError> {
    let path = path
        .canonicalize()
        .map_err(|error| fail("LATENCY_HOST", format!("resolve input storage: {error}")))?;
    let rendered = path
        .to_str()
        .ok_or_else(|| fail("LATENCY_HOST", "input storage path is not UTF-8"))?;
    let source = command_output("findmnt", &["-no", "SOURCE", "-T", rendered], "input mount")?;
    let parent = command_output("lsblk", &["-ndo", "PKNAME", &source], "storage parent")?;
    let device = if parent.is_empty() {
        source
    } else {
        format!("/dev/{parent}")
    };
    command_output("lsblk", &["-ndo", "MODEL", &device], "storage model")
}

#[cfg(any(target_os = "linux", test))]
fn parse_cpu_list(text: &str) -> Result<Vec<u16>, CommandError> {
    let mut cpus = Vec::new();
    for part in text.split(',') {
        let (start, end) = match part.split_once('-') {
            Some((start, end)) => (parse_u16(start, "affinity")?, parse_u16(end, "affinity")?),
            None => {
                let value = parse_u16(part, "affinity")?;
                (value, value)
            }
        };
        if start > end {
            return Err(fail("LATENCY_HOST", "process affinity range descends"));
        }
        for cpu in start..=end {
            if cpus.last().is_some_and(|last| *last >= cpu) {
                return Err(fail(
                    "LATENCY_HOST",
                    "process affinity is not strictly ordered",
                ));
            }
            cpus.push(cpu);
        }
    }
    Ok(cpus)
}

#[cfg(any(target_os = "linux", test))]
fn parse_u16(text: &str, label: &str) -> Result<u16, CommandError> {
    text.parse()
        .map_err(|_| fail("LATENCY_HOST", format!("{label} value is invalid")))
}

#[cfg(any(target_os = "linux", test))]
fn parse_memory_bytes(text: &str) -> Result<u64, CommandError> {
    let line = text
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .ok_or_else(|| fail("LATENCY_HOST", "total memory is missing"))?;
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 2 || fields[1] != "kB" {
        return Err(fail("LATENCY_HOST", "total memory has an unexpected form"));
    }
    fields[0]
        .parse::<u64>()
        .ok()
        .and_then(|value| value.checked_mul(1024))
        .ok_or_else(|| fail("LATENCY_OVERFLOW", "total memory overflow"))
}

fn command_output(program: &str, arguments: &[&str], label: &str) -> Result<String, CommandError> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| fail("LATENCY_HOST", format!("run {label}: {error}")))?;
    if !output.status.success() {
        return Err(fail("LATENCY_HOST", format!("{label} command failed")));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| fail("LATENCY_HOST", format!("{label} output is not UTF-8")))?;
    Ok(text.trim().to_owned())
}

#[derive(Clone, Copy)]
struct OutputIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

#[derive(Default)]
struct PublicationHooks {
    #[cfg(test)]
    fail_write: bool,
    #[cfg(test)]
    fail_file_sync: bool,
    #[cfg(test)]
    fail_directory_sync: bool,
    #[cfg(test)]
    before_final_identity_check: Option<fn(&Path)>,
}

impl PublicationHooks {
    fn write(&mut self, file: &mut File, bytes: &[u8]) -> Result<(), CommandError> {
        #[cfg(test)]
        if self.fail_write {
            file.write_all(&bytes[..bytes.len().min(1)])
                .map_err(|error| {
                    fail("LATENCY_OUTPUT", format!("partial report write: {error}"))
                })?;
            return Err(fail("LATENCY_OUTPUT", "injected report write failure"));
        }
        file.write_all(bytes)
            .map_err(|error| fail("LATENCY_OUTPUT", format!("write report: {error}")))
    }

    fn sync_file(&mut self, file: &File) -> Result<(), CommandError> {
        #[cfg(test)]
        if self.fail_file_sync {
            return Err(fail("LATENCY_OUTPUT", "injected report file sync failure"));
        }
        file.sync_all()
            .map_err(|error| fail("LATENCY_OUTPUT", format!("sync report: {error}")))
    }

    fn sync_directory(&mut self, directory: &File) -> Result<(), CommandError> {
        #[cfg(test)]
        if self.fail_directory_sync {
            return Err(fail(
                "LATENCY_OUTPUT",
                "injected report directory sync failure",
            ));
        }
        directory
            .sync_all()
            .map_err(|error| fail("LATENCY_OUTPUT", format!("sync report directory: {error}")))
    }

    fn before_final_identity_check(&mut self, _path: &Path) {
        #[cfg(test)]
        if let Some(hook) = self.before_final_identity_check {
            hook(_path);
        }
    }
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    publish_new_with_hooks(path, bytes, &mut PublicationHooks::default())
}

fn publish_new_with_hooks(
    path: &Path,
    bytes: &[u8],
    hooks: &mut PublicationHooks,
) -> Result<(), CommandError> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| fail("LATENCY_OUTPUT", format!("create report: {error}")))?;
    let identity = output_identity(&file)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let result = (|| {
        hooks.write(&mut file, bytes)?;
        hooks.sync_file(&file)?;
        require_output_identity(path, identity)?;
        let directory = File::open(parent)
            .map_err(|error| fail("LATENCY_OUTPUT", format!("open report directory: {error}")))?;
        hooks.sync_directory(&directory)?;
        hooks.before_final_identity_check(path);
        require_output_identity(path, identity)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(original) => match cleanup_owned_output(path, parent, &file, identity) {
            Ok(()) => Err(original),
            Err(cleanup) => Err(fail(
                "LATENCY_OUTPUT_CLEANUP",
                format!("{original}; cleanup also failed: {cleanup}"),
            )),
        },
    }
}

#[cfg(unix)]
fn output_identity(file: &File) -> Result<OutputIdentity, CommandError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file
        .metadata()
        .map_err(|error| fail("LATENCY_OUTPUT", format!("inspect held report: {error}")))?;
    if !metadata.file_type().is_file() {
        return Err(fail(
            "LATENCY_OUTPUT",
            "created report is not a regular file",
        ));
    }
    Ok(OutputIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn output_identity(file: &File) -> Result<OutputIdentity, CommandError> {
    if !file
        .metadata()
        .map_err(|error| fail("LATENCY_OUTPUT", format!("inspect held report: {error}")))?
        .file_type()
        .is_file()
    {
        return Err(fail(
            "LATENCY_OUTPUT",
            "created report is not a regular file",
        ));
    }
    Ok(OutputIdentity {})
}

#[cfg(unix)]
fn path_has_output_identity(path: &Path, identity: OutputIdentity) -> Result<bool, CommandError> {
    use std::os::unix::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_file()
            && metadata.dev() == identity.device
            && metadata.ino() == identity.inode),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(fail(
            "LATENCY_OUTPUT",
            format!("inspect report path: {error}"),
        )),
    }
}

#[cfg(not(unix))]
fn path_has_output_identity(path: &Path, _identity: OutputIdentity) -> Result<bool, CommandError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(metadata.file_type().is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(fail(
            "LATENCY_OUTPUT",
            format!("inspect report path: {error}"),
        )),
    }
}

fn require_output_identity(path: &Path, identity: OutputIdentity) -> Result<(), CommandError> {
    if path_has_output_identity(path, identity)? {
        Ok(())
    } else {
        Err(fail(
            "LATENCY_OUTPUT",
            "report path no longer names the created inode",
        ))
    }
}

fn cleanup_owned_output(
    path: &Path,
    parent: &Path,
    file: &File,
    identity: OutputIdentity,
) -> Result<(), CommandError> {
    if path_has_output_identity(path, identity)? {
        fs::remove_file(path)
            .map_err(|error| fail("LATENCY_OUTPUT_CLEANUP", format!("remove report: {error}")))?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                fail(
                    "LATENCY_OUTPUT_CLEANUP",
                    format!("sync report cleanup: {error}"),
                )
            })?;
        return Ok(());
    }
    if output_link_count(file)? == 0 {
        return Ok(());
    }
    Err(fail(
        "LATENCY_OUTPUT_CLEANUP",
        "created report inode moved to an unknown path",
    ))
}

#[cfg(unix)]
fn output_link_count(file: &File) -> Result<u64, CommandError> {
    use std::os::unix::fs::MetadataExt;
    file.metadata()
        .map(|metadata| metadata.nlink())
        .map_err(|error| {
            fail(
                "LATENCY_OUTPUT_CLEANUP",
                format!("inspect report links: {error}"),
            )
        })
}

#[cfg(not(unix))]
fn output_link_count(_file: &File) -> Result<u64, CommandError> {
    Ok(0)
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fail(code: &'static str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(value: u64) -> Vec<u64> {
        vec![value; SAMPLES]
    }

    #[test]
    fn nearest_rank_p50_uses_lower_middle_for_twenty_samples() {
        let mut values: Vec<_> = (1..=20).rev().collect();
        assert_eq!(nearest_rank_p50(&values).expect("p50"), 10);
        values.pop();
        assert!(nearest_rank_p50(&values).is_err());
    }

    #[test]
    fn exact_relative_gate_accepts_its_boundary_and_rejects_one_over() {
        let boundary =
            workload_report(1, samples(210), samples(2_100), u64::MAX).expect("relative boundary");
        assert!(boundary.ten_times_passed);
        assert!(boundary.absolute_limit_passed);
        let over = workload_report(1, samples(210), samples(2_101), u64::MAX)
            .expect("relative over boundary");
        assert!(!over.ten_times_passed);
        assert!(over.absolute_limit_passed);
    }

    #[test]
    fn exact_absolute_gate_accepts_its_boundary_and_rejects_one_over() {
        let boundary =
            workload_report(1, samples(1_000), samples(2_100), 2_100).expect("absolute boundary");
        assert!(boundary.ten_times_passed);
        assert!(boundary.absolute_limit_passed);
        let over = workload_report(1, samples(1_000), samples(2_101), 2_100)
            .expect("absolute over boundary");
        assert!(over.ten_times_passed);
        assert!(!over.absolute_limit_passed);
    }

    #[test]
    fn workload_rejects_ratio_overflow() {
        assert!(workload_report(1, samples(u64::MAX), samples(1), 2_100).is_err());
    }

    #[test]
    fn workload_rejects_wrong_sample_count_alone() {
        assert!(workload_report(1, vec![1; 19], samples(1), 2_100).is_err());
    }

    #[test]
    fn workload_rejects_unknown_request_count_alone() {
        assert!(workload_report(2, samples(1), samples(1), 2_100).is_err());
    }

    #[test]
    fn manifest_parser_rejects_order_count_workload_and_duplicates() {
        let valid = include_bytes!("../../../planning/artifacts/002-query-manifest.tsv");
        assert_eq!(parse_queries(valid).expect("checked manifest").len(), 100);
        let text = std::str::from_utf8(valid).expect("UTF-8");
        assert!(parse_queries(text.replacen("1\tchrX", "2\tchrX", 1).as_bytes()).is_err());
        assert!(
            parse_queries(
                text.replacen("primary-distinct-gene-filtered", "other", 1)
                    .as_bytes()
            )
            .is_err()
        );
        let short = format!(
            "{}\n",
            text.lines().take(100).collect::<Vec<_>>().join("\n")
        );
        let count_error = parse_queries(short.as_bytes()).expect_err("99 rows must fail");
        assert_eq!(
            count_error.message,
            "query manifest must contain exactly 100 requests"
        );
        let first = text.lines().nth(1).expect("first row");
        let second = text.lines().nth(2).expect("second row");
        let duplicate = text.replacen(second, &format!("2{}", &first[1..]), 1);
        assert!(parse_queries(duplicate.as_bytes()).is_err());
    }

    #[test]
    fn affinity_and_memory_parsers_fail_closed() {
        assert_eq!(
            parse_cpu_list("0-3,5").expect("CPU list"),
            vec![0, 1, 2, 3, 5]
        );
        assert!(parse_cpu_list("1-0").is_err());
        assert!(parse_cpu_list("0,0").is_err());
        assert_eq!(
            parse_memory_bytes("MemTotal:       1024 kB\n").expect("memory"),
            1_048_576
        );
        assert!(parse_memory_bytes("MemTotal: 1024 MB\n").is_err());
        assert_eq!(
            parse_ubuntu_release("ID=ubuntu\nVERSION_ID=\"24.04\"\n").expect("Ubuntu"),
            "24.04"
        );
        assert!(parse_ubuntu_release("ID=debian\nVERSION_ID=\"24.04\"\n").is_err());
        assert!(parse_ubuntu_release("ID=ubuntu\nVERSION_ID=\"22.04\"\n").is_err());
    }

    #[test]
    fn canonical_report_has_exact_ratio_operands_and_round_trips() {
        let report = workload_report(10, samples(100), samples(500), 19_640).expect("report");
        let bytes = serde_jcs::to_vec(&report).expect("canonical JSON");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
        assert_eq!(value["candidate_to_fixed_ratio"]["candidate_p50_ns"], 500);
        assert_eq!(value["candidate_to_fixed_ratio"]["fixed_p50_ns"], 100);
        assert_eq!(serde_jcs::to_vec(&value).expect("re-encode"), bytes);
    }

    #[test]
    fn answer_comparison_rejects_any_record_or_ambiguity_difference() {
        assert!(require_matching_answer(&(vec![1], vec![2]), &(vec![1], vec![2]), 1).is_ok());
        assert!(require_matching_answer(&(vec![1], vec![2]), &(vec![1], vec![3]), 1).is_err());
        assert!(require_matching_answer(&(vec![1], vec![2]), &(vec![3], vec![2]), 1).is_err());
    }

    #[test]
    fn production_runner_order_alternates_from_fixed_first_for_both_phases() {
        use std::{cell::RefCell, rc::Rc};
        for sample in 0..WARMUPS.max(SAMPLES) {
            let trace = Rc::new(RefCell::new(Vec::new()));
            let fixed_trace = Rc::clone(&trace);
            let candidate_trace = Rc::clone(&trace);
            let values = run_pair_with(
                sample,
                || {
                    fixed_trace
                        .borrow_mut()
                        .push(ReaderOrder::FixedThenCandidate);
                    Ok(1_u8)
                },
                || {
                    candidate_trace
                        .borrow_mut()
                        .push(ReaderOrder::CandidateThenFixed);
                    Ok(2_u8)
                },
            )
            .expect("production pair runner");
            assert_eq!(values, (1, 2));
            let expected = if sample.is_multiple_of(2) {
                vec![
                    ReaderOrder::FixedThenCandidate,
                    ReaderOrder::CandidateThenFixed,
                ]
            } else {
                vec![
                    ReaderOrder::CandidateThenFixed,
                    ReaderOrder::FixedThenCandidate,
                ]
            };
            assert_eq!(*trace.borrow(), expected, "sample {sample}");
        }
    }

    #[test]
    fn executable_commit_binding_rejects_stale_binary_and_wrong_checkout() {
        let expected = "1111111111111111111111111111111111111111";
        let other = "2222222222222222222222222222222222222222";
        assert!(validate_commit_binding(expected, expected, expected, "true").is_ok());
        assert!(validate_commit_binding(expected, other, expected, "true").is_err());
        assert!(validate_commit_binding(expected, expected, other, "true").is_err());
        assert!(validate_commit_binding(expected, expected, expected, "false").is_err());
        assert!(validate_commit_binding(expected, expected, expected, "unavailable").is_err());
    }

    #[test]
    fn time_reader_checks_totals_and_propagates_lookup_errors() {
        let query = Query {
            snv: Grch38Snv::new(
                "chr1".parse().expect("contig"),
                pangopup_core::GenomicPosition::new(10).expect("position"),
                pangopup_core::DnaBase::A,
                pangopup_core::DnaBase::C,
            )
            .expect("SNV"),
            gene: "ENSG00000000001".parse().expect("gene"),
        };
        assert!(time_reader(&[query], (1, 1), |_| Ok((vec![1_u8], vec![2_u8]))).is_ok());
        assert!(time_reader(&[query], (2, 1), |_| Ok((vec![1_u8], vec![2_u8]))).is_err());
        assert!(
            time_reader::<u8, u8>(&[query], (0, 0), |_| {
                Err(fail("LATENCY_FIXED", "injected lookup failure"))
            })
            .is_err()
        );
    }

    #[test]
    fn production_pair_runner_uses_actual_fixed_and_sparse_readers() {
        use pangopup_core::{
            DnaBase, GenomicPosition, PangolinScore, RelativePosition, ScoreMagnitude,
        };
        use pangopup_index::{
            InputAlternative, InputLocus, OrdinaryInputLocus, StreamingIndexWriter,
            sparse_writer::SparseIndexWriter,
        };

        let directory = tempfile::tempdir().expect("temporary directory");
        let zero = ScoreMagnitude::new(0).expect("zero");
        let default_position = RelativePosition::new(-50).expect("position");
        let score = PangolinScore::new(zero, default_position, zero, default_position);
        let gene: EnsemblGeneId = "ENSG00000000001".parse().expect("gene");
        let contig = "chr1".parse().expect("contig");
        let position = GenomicPosition::new(10).expect("position");
        let input = [InputLocus::Ordinary(OrdinaryInputLocus {
            gene,
            contig,
            position,
            reference: DnaBase::A,
            alternatives: [DnaBase::C, DnaBase::G, DnaBase::T]
                .map(|alternate| InputAlternative { alternate, score }),
        })];

        let fixed_path = directory.path().join("fixed.pgi");
        let mut fixed_writer =
            StreamingIndexWriter::create(&directory.path().join("fixed.scratch"))
                .expect("fixed writer");
        fixed_writer.push_gene(&input).expect("fixed gene");
        fixed_writer.finish(&fixed_path).expect("fixed finish");

        let sparse_path = directory.path().join("sparse.pgi");
        let mut sparse_writer = SparseIndexWriter::create(&directory.path().join("sparse.scratch"))
            .expect("sparse writer");
        sparse_writer.push_gene(&input).expect("sparse gene");
        sparse_writer.finish(&sparse_path).expect("sparse finish");

        let fixed = IndexReader::open(&fixed_path).expect("fixed reader");
        let candidate = SparseIndexReader::open(&sparse_path).expect("sparse reader");
        let queries = [Query {
            snv: Grch38Snv::new(contig, position, DnaBase::A, DnaBase::G).expect("SNV"),
            gene,
        }];
        assert_eq!(
            require_equal_answers(&fixed, &candidate, &queries).expect("equal answers"),
            vec![(1, 0)]
        );
        assert!(run_pair(0, &fixed, &candidate, &queries, (1, 0)).is_ok());
        assert!(run_pair(1, &fixed, &candidate, &queries, (1, 0)).is_ok());
    }

    #[test]
    fn exact_regular_identity_rejects_size_digest_and_nonfiles() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let member = directory.path().join("member");
        fs::write(&member, b"identity").expect("write member");
        let digest = hex_digest(b"identity");
        let (held, _) = open_exact_regular(&member, "test", 8, &digest).expect("exact member");
        assert_eq!(
            read_held(&held, 8, "test").expect("read held member"),
            b"identity"
        );
        assert!(open_exact_regular(&member, "test", 7, &digest).is_err());
        assert!(open_exact_regular(&member, "test", 8, &hex_digest(b"different")).is_err());
        assert!(open_exact_regular(directory.path(), "test", 8, &digest).is_err());
    }

    #[test]
    fn report_publication_is_create_new_and_preserves_the_first_report() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let report = directory.path().join("report.json");
        publish_new(&report, b"first").expect("first publication");
        assert!(publish_new(&report, b"second").is_err());
        assert_eq!(fs::read(report).expect("retained report"), b"first");
    }

    #[test]
    fn report_publication_removes_owned_output_after_write_and_sync_failures() {
        let directory = tempfile::tempdir().expect("temporary directory");
        for (name, mut hooks) in [
            (
                "write.json",
                PublicationHooks {
                    fail_write: true,
                    ..PublicationHooks::default()
                },
            ),
            (
                "file-sync.json",
                PublicationHooks {
                    fail_file_sync: true,
                    ..PublicationHooks::default()
                },
            ),
            (
                "directory-sync.json",
                PublicationHooks {
                    fail_directory_sync: true,
                    ..PublicationHooks::default()
                },
            ),
        ] {
            let path = directory.path().join(name);
            assert!(publish_new_with_hooks(&path, b"report", &mut hooks).is_err());
            assert!(!path.exists(), "owned failed output remained at {name}");
        }
    }

    #[test]
    fn report_publication_never_deletes_a_replacement() {
        fn replace(path: &Path) {
            fs::remove_file(path).expect("unlink owned report");
            fs::write(path, b"replacement").expect("write replacement");
        }
        let directory = tempfile::tempdir().expect("temporary directory");
        let report = directory.path().join("report.json");
        let mut hooks = PublicationHooks {
            before_final_identity_check: Some(replace),
            ..PublicationHooks::default()
        };
        assert!(publish_new_with_hooks(&report, b"owned", &mut hooks).is_err());
        assert_eq!(
            fs::read(report).expect("replacement report"),
            b"replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn report_publication_rejects_a_symlink_to_the_moved_owned_inode() {
        fn replace_with_symlink(path: &Path) {
            use std::os::unix::fs::symlink;
            let moved = path.with_extension("moved");
            fs::rename(path, &moved).expect("move owned report");
            symlink(&moved, path).expect("link replacement path to moved inode");
        }
        let directory = tempfile::tempdir().expect("temporary directory");
        let report = directory.path().join("report.json");
        let moved = report.with_extension("moved");
        let mut hooks = PublicationHooks {
            before_final_identity_check: Some(replace_with_symlink),
            ..PublicationHooks::default()
        };
        assert!(publish_new_with_hooks(&report, b"owned", &mut hooks).is_err());
        assert!(
            fs::symlink_metadata(&report)
                .expect("replacement metadata")
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&moved).expect("moved owned report"), b"owned");
    }
}
