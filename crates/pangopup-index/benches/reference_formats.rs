use pangopup_core::Grch38Contig;
use pangopup_index::reference_candidates::{
    CandidateCodec, CandidateReader, SelectionCandidate, SelectionResult, publish_benchmark_report,
    select_candidate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::BTreeSet,
    env,
    fs::{self, File},
    hint::black_box,
    io::Write,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

const CONTRACT_SHA256: &str = "0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee";
const EXPECTED_PROFILE: &str = "refseq-grch38p14-compat-six-contigs-v1";
const EXPECTED_SOURCE_BYTES: u64 = 671_294_255;
const EXPECTED_SOURCE_SHA256: &str =
    "81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb";
const EXPECTED_CORPUS_PROFILE: &str = "pangolin-1.0.2-5cf94b8-grch38-v1";
const EXPECTED_MANIFEST_BYTES: u64 = 5_337;
const EXPECTED_MANIFEST_SHA256: &str =
    "fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b";
const EXPECTED_CASES_BYTES: u64 = 220_071;
const EXPECTED_CASES_SHA256: &str =
    "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const CASE_IDS: [&str; 14] = [
    "M01-snv-cd4-precomputed",
    "M02-snv-wrap53-tp53-precomputed",
    "M03-snv-afap1l2-precomputed",
    "M04-snv-grk1-precomputed",
    "M05-snv-same-strand-overlap",
    "M06-snv-gene-start-plus-one",
    "M07-mnv-plus",
    "M08-mnv-both-strands",
    "M09-insertion-short-plus",
    "M10-insertion-short-both",
    "M11-insertion-long-overlap",
    "M12-deletion-short-plus",
    "M13-deletion-short-both",
    "M14-deletion-ref100-overlap",
];
const ORDERS: [[CandidateCodec; 3]; 5] = [
    [
        CandidateCodec::Ascii8,
        CandidateCodec::Iupac4,
        CandidateCodec::Acgt2RleV1,
    ],
    [
        CandidateCodec::Iupac4,
        CandidateCodec::Acgt2RleV1,
        CandidateCodec::Ascii8,
    ],
    [
        CandidateCodec::Acgt2RleV1,
        CandidateCodec::Ascii8,
        CandidateCodec::Iupac4,
    ],
    [
        CandidateCodec::Ascii8,
        CandidateCodec::Acgt2RleV1,
        CandidateCodec::Iupac4,
    ],
    [
        CandidateCodec::Acgt2RleV1,
        CandidateCodec::Iupac4,
        CandidateCodec::Ascii8,
    ],
];

struct CountingAllocator;
static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: every operation delegates to the process System allocator; the
// atomics only observe allocation counts while the single-threaded harness has
// explicitly enabled measurement.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATION_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        // SAFETY: this delegates the unchanged valid layout to System.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: this delegates the original allocation and layout to System.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACK.load(Ordering::Relaxed) {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATION_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        // SAFETY: this delegates the original allocation and requested size to System.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    profile: String,
    source: Identity,
    corpus: Corpus,
    container: Container,
    members: Vec<Member>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    profile: String,
    manifest_bytes: u64,
    manifest_sha256: String,
    cases_bytes: u64,
    cases_sha256: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Container {
    schema: String,
    page_bytes: u64,
    contigs: Vec<u8>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Member {
    codec: CandidateCodec,
    filename: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    kind: String,
    input: Input,
    context: Context,
}
#[derive(Deserialize)]
struct Input {
    contig: String,
}
#[derive(Deserialize)]
struct Context {
    start_1based: u64,
    bases: String,
    sha256: String,
}

struct BenchmarkContext {
    contig: Grch38Contig,
    start_1based: u64,
    bases: String,
    sha256: String,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    contract_sha256: &'static str,
    candidate_set_sha256: String,
    source: Identity,
    corpus: CorpusReport,
    environment: Environment,
    method: Method,
    candidates: Vec<CandidateReport>,
    process: Process,
    selection: SelectionResult,
}
#[derive(Serialize)]
struct CorpusReport {
    manifest_bytes: u64,
    manifest_sha256: String,
    cases_bytes: u64,
    cases_sha256: String,
}
#[derive(Serialize)]
struct Environment {
    rustc: String,
    target: String,
    os: String,
    kernel: String,
    cpu: String,
    logical_cpus: u64,
    power: String,
    affinity: String,
}
#[derive(Serialize)]
struct Method {
    page_bytes: u64,
    rounds: u64,
    warmups_per_round: u64,
    operations_per_round: u64,
    quantile: &'static str,
    candidate_orders: Vec<Vec<CandidateCodec>>,
}
#[derive(Serialize)]
struct CandidateReport {
    codec: CandidateCodec,
    member_bytes: u64,
    member_sha256: String,
    open_ns: Vec<u64>,
    round_p50_ns: Vec<u64>,
    round_p95_ns: Vec<u64>,
    headline_p50_ns: u64,
    headline_p95_ns: u64,
    allocation_calls_per_copy: u64,
    allocation_bytes_per_copy: u64,
    logical_bases: u64,
    unique_pages: Vec<u64>,
    unique_page_count: u64,
    zstd_bytes: u64,
}

struct CandidateTiming {
    codec: CandidateCodec,
    path: PathBuf,
    member_bytes: u64,
    member_sha256: String,
    open_ns: Vec<u64>,
    retained_ns: Vec<Vec<u64>>,
    allocation_calls_per_copy: u64,
    allocation_bytes_per_copy: u64,
}
#[derive(Serialize)]
struct Process {
    maximum_rss_bytes: u64,
    minor_faults: u64,
    major_faults: u64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("reference benchmark failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), &'static str> {
    let candidates = absolute_env("PANGOPUP_REFERENCE_CANDIDATES")?;
    let corpus = absolute_env("PANGOPUP_REFERENCE_CORPUS")?;
    let report_path = absolute_env("PANGOPUP_REFERENCE_REPORT")?;
    if report_path.exists() {
        return Err("report already exists");
    }
    if fs::canonicalize(&candidates).map_err(|_| "candidate path")?
        == fs::canonicalize(&corpus).map_err(|_| "corpus path")?
    {
        return Err("candidate and corpus paths alias");
    }
    validate_candidate_directory(&candidates)?;
    validate_corpus_directory(&corpus)?;
    let manifest_bytes = bounded_read(&candidates.join("manifest.json"), 16_384)?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| "candidate manifest JSON")?;
    if serde_jcs::to_vec(&manifest).map_err(|_| "candidate manifest canonicalization")?
        != manifest_bytes
    {
        return Err("candidate manifest is not canonical");
    }
    if manifest.schema != "pangopup-reference-candidates-v1"
        || manifest.profile != EXPECTED_PROFILE
        || manifest.source.bytes != EXPECTED_SOURCE_BYTES
        || manifest.source.sha256 != EXPECTED_SOURCE_SHA256
        || manifest.corpus.schema != "pangopup-compat-v1"
        || manifest.corpus.profile != EXPECTED_CORPUS_PROFILE
        || manifest.corpus.manifest_bytes != EXPECTED_MANIFEST_BYTES
        || manifest.corpus.manifest_sha256 != EXPECTED_MANIFEST_SHA256
        || manifest.corpus.cases_bytes != EXPECTED_CASES_BYTES
        || manifest.corpus.cases_sha256 != EXPECTED_CASES_SHA256
        || manifest.container.schema != "pgrben01-v1"
        || manifest.container.page_bytes != 4096
        || manifest.container.contigs != [3, 10, 12, 13, 17, 25]
        || manifest.members.len() != 3
    {
        return Err("unexpected candidate profile");
    }
    let corpus_manifest = bounded_read(&corpus.join("manifest.json"), 128 * 1024)?;
    if corpus_manifest.len() as u64 != EXPECTED_MANIFEST_BYTES
        || hex_sha(&corpus_manifest) != EXPECTED_MANIFEST_SHA256
    {
        return Err("corpus manifest identity");
    }
    let cases_bytes = bounded_read(&corpus.join("cases.jsonl"), 4_000_000)?;
    if cases_bytes.len() as u64 != EXPECTED_CASES_BYTES
        || hex_sha(&cases_bytes) != EXPECTED_CASES_SHA256
    {
        return Err("corpus cases identity");
    }
    let contexts = contexts(&cases_bytes)?;
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATION_BYTES.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::SeqCst);
    black_box(());
    TRACK.store(false, Ordering::SeqCst);
    if ALLOCATION_CALLS.load(Ordering::Relaxed) != 0
        || ALLOCATION_BYTES.load(Ordering::Relaxed) != 0
    {
        return Err("empty allocation control failed");
    }
    let candidate_set_sha256 = hex_sha(&manifest_bytes);
    for (index, codec) in CandidateCodec::ALL.iter().enumerate() {
        let member = manifest.members.get(index).ok_or("candidate member")?;
        if member.codec != *codec || member.filename != codec.filename() {
            return Err("candidate member order");
        }
        let path = candidates.join(&member.filename);
        if fs::metadata(&path).map_err(|_| "candidate metadata")?.len() != member.bytes
            || file_sha(&path)? != member.sha256
        {
            return Err("candidate member identity");
        }
        let reader = CandidateReader::open(&path).map_err(|_| "candidate open preflight")?;
        if reader.codec() != *codec {
            return Err("candidate codec preflight");
        }
        reader
            .inspect_payload()
            .map_err(|_| "candidate payload preflight")?;
        verify_contexts(&reader, &contexts)?;
    }
    let timings = run_timing_rounds(&candidates, &manifest, &contexts)?;
    let reports = finalize_candidate_reports(timings, &contexts)?;
    let selection_inputs: Vec<_> = reports
        .iter()
        .map(|report| SelectionCandidate {
            codec: report.codec,
            headline_p50_ns: report.headline_p50_ns,
            headline_p95_ns: report.headline_p95_ns,
            unique_page_count: report.unique_page_count,
            member_bytes: report.member_bytes,
            zstd_bytes: report.zstd_bytes,
            evidence_valid: true,
        })
        .collect();
    let selection = select_candidate(&selection_inputs);
    let report = Report {
        schema: "pangopup-reference-format-benchmark-v1",
        contract_sha256: CONTRACT_SHA256,
        candidate_set_sha256,
        source: manifest.source,
        corpus: CorpusReport {
            manifest_bytes: manifest.corpus.manifest_bytes,
            manifest_sha256: manifest.corpus.manifest_sha256,
            cases_bytes: manifest.corpus.cases_bytes,
            cases_sha256: manifest.corpus.cases_sha256,
        },
        environment: environment(),
        method: Method {
            page_bytes: 4096,
            rounds: 5,
            warmups_per_round: 20,
            operations_per_round: 10_000,
            quantile: "nearest-rank",
            candidate_orders: ORDERS.iter().map(|order| order.to_vec()).collect(),
        },
        candidates: reports,
        process: process_usage(),
        selection,
    };
    let bytes = serde_jcs::to_vec(&report).map_err(|_| "report serialization")?;
    publish_benchmark_report(&report_path, &bytes).map_err(|_| "report publication")
}

// TIMING-ROUNDS-BEGIN: the structural control requires this section to contain
// only open, warmup, and retained-operation work.
fn run_timing_rounds(
    candidates: &Path,
    manifest: &Manifest,
    contexts: &[BenchmarkContext],
) -> Result<Vec<CandidateTiming>, &'static str> {
    let mut timings = CandidateCodec::ALL
        .iter()
        .enumerate()
        .map(|(index, codec)| {
            let member = manifest.members.get(index).ok_or("candidate member")?;
            Ok(CandidateTiming {
                codec: *codec,
                path: candidates.join(&member.filename),
                member_bytes: member.bytes,
                member_sha256: member.sha256.clone(),
                open_ns: vec![0; 5],
                retained_ns: (0..5).map(|_| vec![0; 10_000]).collect(),
                allocation_calls_per_copy: 0,
                allocation_bytes_per_copy: 0,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let maximum = contexts
        .iter()
        .map(|case| case.bases.len())
        .max()
        .ok_or("empty contexts")?;
    let mut destination = vec![0_u8; maximum];
    destination.fill(0);

    for (round, order) in ORDERS.iter().enumerate() {
        for codec in order {
            let index = CandidateCodec::ALL
                .iter()
                .position(|candidate| candidate == codec)
                .ok_or("candidate order")?;
            let timing = timings.get_mut(index).ok_or("candidate timing")?;
            let opened = Instant::now();
            let reader = CandidateReader::open(&timing.path).map_err(|_| "candidate open")?;
            let open_ns = nanos(opened.elapsed().as_nanos())?;
            for operation in 0..20 {
                copy_unmeasured(&reader, &contexts[operation % 14], &mut destination)?;
            }
            ALLOCATION_CALLS.store(0, Ordering::Relaxed);
            ALLOCATION_BYTES.store(0, Ordering::Relaxed);
            for operation in 0..10_000 {
                let case = &contexts[operation % 14];
                let output = &mut destination[..case.bases.len()];
                TRACK.store(true, Ordering::SeqCst);
                let start = Instant::now();
                let copied = reader.copy_window(case.contig, case.start_1based, output);
                let elapsed = start.elapsed().as_nanos();
                TRACK.store(false, Ordering::SeqCst);
                copied.map_err(|_| "context copy")?;
                timing.retained_ns[round][operation] = nanos(elapsed)?;
                black_box(Sha256::digest(output));
            }
            let calls = ALLOCATION_CALLS.load(Ordering::Relaxed);
            let bytes = ALLOCATION_BYTES.load(Ordering::Relaxed);
            if calls != 0 || bytes != 0 {
                return Err("measured copy allocated");
            }
            timing.open_ns[round] = open_ns;
            timing.allocation_calls_per_copy = calls;
            timing.allocation_bytes_per_copy = bytes;
        }
    }
    Ok(timings)
}
// TIMING-ROUNDS-END

// REPORT-WORK-BEGIN: this function is called only after every timing round has
// returned, so page tracing, quantiles, and whole-member compression cannot
// perturb a later timing block.
fn finalize_candidate_reports(
    timings: Vec<CandidateTiming>,
    contexts: &[BenchmarkContext],
) -> Result<Vec<CandidateReport>, &'static str> {
    let logical_bases = contexts.iter().map(|case| case.bases.len() as u64).sum();
    let mut reports = Vec::with_capacity(timings.len());
    for mut timing in timings {
        let mut round_p50_ns = Vec::with_capacity(5);
        let mut round_p95_ns = Vec::with_capacity(5);
        for retained in &mut timing.retained_ns {
            retained.sort_unstable();
            round_p50_ns.push(retained[4_999]);
            round_p95_ns.push(retained[9_499]);
        }
        let mut p50 = round_p50_ns.clone();
        p50.sort_unstable();
        let mut p95 = round_p95_ns.clone();
        p95.sort_unstable();

        let reader = CandidateReader::open(&timing.path).map_err(|_| "candidate report open")?;
        let mut pages: BTreeSet<u64> = reader.open_trace_pages().into_iter().collect();
        for case in contexts {
            pages.extend(
                reader
                    .trace_window(case.contig, case.start_1based, case.bases.len())
                    .map_err(|_| "logical trace")?,
            );
        }
        let unique_pages: Vec<u64> = pages.into_iter().collect();
        reports.push(CandidateReport {
            codec: timing.codec,
            member_bytes: timing.member_bytes,
            member_sha256: timing.member_sha256,
            open_ns: timing.open_ns,
            round_p50_ns,
            round_p95_ns,
            headline_p50_ns: p50[2],
            headline_p95_ns: p95[2],
            allocation_calls_per_copy: timing.allocation_calls_per_copy,
            allocation_bytes_per_copy: timing.allocation_bytes_per_copy,
            logical_bases,
            unique_page_count: unique_pages.len() as u64,
            unique_pages,
            zstd_bytes: zstd_size(&timing.path)?,
        });
    }
    Ok(reports)
}
// REPORT-WORK-END

fn copy_unmeasured(
    reader: &CandidateReader,
    case: &BenchmarkContext,
    destination: &mut [u8],
) -> Result<(), &'static str> {
    let output = &mut destination[..case.bases.len()];
    reader
        .copy_window(case.contig, case.start_1based, output)
        .map_err(|_| "context copy")
}

fn contexts(bytes: &[u8]) -> Result<Vec<BenchmarkContext>, &'static str> {
    let text = std::str::from_utf8(bytes).map_err(|_| "cases UTF-8")?;
    let mut result = Vec::new();
    for (index, line) in text.lines().take(14).enumerate() {
        let case: Case = serde_json::from_str(line).map_err(|_| "case JSON")?;
        if case.id != CASE_IDS[index]
            || case.kind != "model"
            || hex_sha(case.context.bases.as_bytes()) != case.context.sha256
        {
            return Err("context identity");
        }
        result.push(BenchmarkContext {
            contig: Grch38Contig::from_str(&case.input.contig).map_err(|_| "context contig")?,
            start_1based: case.context.start_1based,
            bases: case.context.bases,
            sha256: case.context.sha256,
        });
    }
    if result.len() != 14 {
        return Err("context count");
    }
    Ok(result)
}

fn verify_contexts(
    reader: &CandidateReader,
    contexts: &[BenchmarkContext],
) -> Result<(), &'static str> {
    for case in contexts {
        let mut bytes = vec![0; case.bases.len()];
        reader
            .copy_window(case.contig, case.start_1based, &mut bytes)
            .map_err(|_| "context copy")?;
        if bytes != case.bases.as_bytes() || hex_sha(&bytes) != case.sha256 {
            return Err("context mismatch");
        }
    }
    Ok(())
}

fn zstd_size(path: &Path) -> Result<u64, &'static str> {
    let size = fs::metadata(path).map_err(|_| "zstd input metadata")?.len();
    let mut encoder = zstd::stream::Encoder::new(CountingSink(0), 9).map_err(|_| "zstd encoder")?;
    encoder
        .include_checksum(true)
        .map_err(|_| "zstd checksum")?;
    encoder
        .include_contentsize(true)
        .map_err(|_| "zstd content size")?;
    encoder
        .include_dictid(false)
        .map_err(|_| "zstd dictionary ID")?;
    encoder
        .long_distance_matching(false)
        .map_err(|_| "zstd long-distance matching")?;
    encoder.multithread(0).map_err(|_| "zstd workers")?;
    encoder
        .set_pledged_src_size(Some(size))
        .map_err(|_| "zstd pledged size")?;
    let mut input = File::open(path).map_err(|_| "zstd input")?;
    std::io::copy(&mut input, &mut encoder).map_err(|_| "zstd encode")?;
    Ok(encoder.finish().map_err(|_| "zstd finish")?.0)
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

fn validate_candidate_directory(root: &Path) -> Result<(), &'static str> {
    let metadata = fs::symlink_metadata(root).map_err(|_| "candidate directory")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("candidate directory type");
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|_| "candidate directory read")? {
        let entry = entry.map_err(|_| "candidate directory entry")?;
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|_| "candidate member metadata")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err("candidate member type");
        }
        names.insert(entry.file_name());
    }
    let expected: BTreeSet<_> = [
        "manifest.json",
        "ascii8.pgr",
        "iupac4.pgr",
        "acgt2-rle-v1.pgr",
    ]
    .into_iter()
    .map(std::ffi::OsString::from)
    .collect();
    if names != expected {
        return Err("candidate directory members");
    }
    Ok(())
}

fn validate_corpus_directory(root: &Path) -> Result<(), &'static str> {
    let metadata = fs::symlink_metadata(root).map_err(|_| "corpus directory")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("corpus directory type");
    }
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(root).map_err(|_| "corpus directory read")? {
        let entry = entry.map_err(|_| "corpus directory entry")?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|_| "corpus member metadata")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err("corpus member type");
        }
        names.insert(entry.file_name());
    }
    let expected: BTreeSet<_> = ["NOTICE", "cases.jsonl", "manifest.json"]
        .into_iter()
        .map(std::ffi::OsString::from)
        .collect();
    if names != expected {
        return Err("corpus directory members");
    }
    Ok(())
}

fn bounded_read(path: &Path, maximum: u64) -> Result<Vec<u8>, &'static str> {
    let before = fs::symlink_metadata(path).map_err(|_| "bounded file metadata")?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() > maximum
    {
        return Err("bounded file identity");
    }
    let bytes = fs::read(path).map_err(|_| "bounded file read")?;
    let after = fs::metadata(path).map_err(|_| "bounded file metadata")?;
    if bytes.len() as u64 != before.len() || after.len() != before.len() || after.nlink() != 1 {
        return Err("bounded file changed");
    }
    Ok(bytes)
}

fn absolute_env(name: &str) -> Result<PathBuf, &'static str> {
    let value = env::var(name).map_err(|_| "missing or non-Unicode benchmark environment")?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("benchmark paths must be absolute");
    }
    Ok(path)
}
fn file_sha(path: &Path) -> Result<String, &'static str> {
    let mut file = File::open(path).map_err(|_| "hash input")?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|_| "hash read")?;
    Ok(format!("{:x}", hasher.finalize()))
}
fn hex_sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn nanos(value: u128) -> Result<u64, &'static str> {
    u64::try_from(value).map_err(|_| "timer overflow")
}

fn environment() -> Environment {
    let (rustc, target) = rustc_identity();
    Environment {
        rustc,
        target,
        os: env::consts::OS.into(),
        kernel: read_trimmed("/proc/sys/kernel/osrelease"),
        cpu: cpu_name(),
        logical_cpus: std::thread::available_parallelism().map_or(0, |value| value.get() as u64),
        power: read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
        affinity: read_trimmed("/proc/self/status")
            .lines()
            .find_map(|line| line.strip_prefix("Cpus_allowed_list:\t"))
            .unwrap_or("unknown")
            .into(),
    }
}

fn rustc_identity() -> (String, String) {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output();
    let Ok(output) = output else {
        return ("unknown".into(), "unknown".into());
    };
    if !output.status.success() {
        return ("unknown".into(), "unknown".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.lines().next().unwrap_or("unknown").to_owned();
    let target = text
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or("unknown")
        .to_owned();
    (version, target)
}
fn read_trimmed(path: &str) -> String {
    fs::read_to_string(path)
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|_| "unknown".into())
}
fn cpu_name() -> String {
    read_trimmed("/proc/cpuinfo")
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| key.trim() == "model name")
                .map(|(_, value)| value.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn process_usage() -> Process {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied rusage structure on success.
    let success = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0;
    if !success {
        return Process {
            maximum_rss_bytes: 0,
            minor_faults: 0,
            major_faults: 0,
        };
    }
    // SAFETY: success above proves the structure was initialized.
    let usage = unsafe { usage.assume_init() };
    Process {
        maximum_rss_bytes: (usage.ru_maxrss as u64).saturating_mul(1024),
        minor_faults: usage.ru_minflt as u64,
        major_faults: usage.ru_majflt as u64,
    }
}
