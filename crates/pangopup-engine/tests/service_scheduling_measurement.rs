use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, Grch38Variant, LookupResult,
    ReferenceProvider, ScoreProvider,
};
use pangopup_engine::VariantScorer;
use pangopup_index::{
    BundleOpen as SnvBundleOpen, mask::MaskDomainsOpen, reference::ReferenceBundleOpen,
};
use pangopup_model::{CpuPolicy, ModelKernel, ONNX_RUNTIME_VERSION, ORT_CRATE_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Barrier, OnceLock,
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const MODEL_ID: &str = "sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43";
const MODEL_PROFILE: &str = "pangolin-1.0.2-5cf94b8-onnx-cpu-v1";
const REFERENCE_ID: &str =
    "sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f";
const REFERENCE_MEMBER: &str =
    "sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82";
const MASK_BYTES: u64 = 6_703_320;
const MASK_SHA256: &str = "714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702";
const SNV_ID: &str = "sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3";
const COMPAT_MANIFEST_SHA256: &str =
    "fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b";
const COMPAT_CASES_BYTES: u64 = 220_071;
const COMPAT_CASES_SHA256: &str =
    "2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8";
const EXPECTED_CPU: &str = "AMD Ryzen 7 5825U with Radeon Graphics";
const EXPECTED_ONLINE_CPUS: &str = "0-15";
const EXPECTED_TOPOLOGY: &str = "package0:cpu0-15:cores0-7:smt-pairs";
const CONCURRENT_IDS: [&str; 8] = [
    "M07-mnv-plus",
    "M08-mnv-both-strands",
    "M09-insertion-short-plus",
    "M10-insertion-short-both",
    "M11-insertion-long-overlap",
    "M12-deletion-short-plus",
    "M13-deletion-short-both",
    "M14-deletion-ref100-overlap",
];

#[derive(Clone, Deserialize)]
struct CorpusCase {
    id: String,
    input: ModelInput,
    strands: Vec<StrandCase>,
}

#[derive(Clone, Deserialize)]
struct ModelInput {
    contig: String,
    position: u32,
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "alt")]
    alternate: String,
}

#[derive(Clone, Deserialize)]
struct StrandCase {
    dtype: String,
    expected: ExpectedModel,
}

#[derive(Clone, Deserialize)]
struct ExpectedModel {
    masked: Vec<ExpectedModelScore>,
}

#[derive(Clone, Deserialize)]
struct ExpectedModelScore {
    gene: String,
    gain_bits: String,
    gain_position: i16,
    loss_bits: String,
    loss_position: i16,
}

#[derive(Deserialize)]
struct ExpectedSnvLine {
    status: String,
    records: Vec<ExpectedSnvScore>,
}

#[derive(Deserialize)]
struct ExpectedSnvScore {
    gene: String,
    gain_score: String,
    gain_position: i16,
    loss_score: String,
    loss_position: i16,
}

#[derive(Clone)]
struct SnvCase {
    snv: Grch38Snv,
    gene: EnsemblGeneId,
    expected: Vec<(EnsemblGeneId, String, i16, String, i16)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
struct Candidate {
    budget: usize,
    workers: usize,
    threads: usize,
}

impl Candidate {
    fn parse(value: &str) -> Result<Self, &'static str> {
        let (workers, threads) = value.split_once('x').ok_or("candidate must be W×T")?;
        let workers = workers.parse::<usize>().map_err(|_| "invalid workers")?;
        let threads = threads.parse::<usize>().map_err(|_| "invalid threads")?;
        let budget = workers.checked_mul(threads).ok_or("candidate overflow")?;
        let candidate = Self {
            budget,
            workers,
            threads,
        };
        if !ALL_CANDIDATES.contains(&candidate) {
            return Err("candidate is not in Ticket 040's closed matrix");
        }
        Ok(candidate)
    }

    fn name(self) -> String {
        format!("{}x{}", self.workers, self.threads)
    }

    fn policy(self) -> CpuPolicy {
        match self.threads {
            1 => CpuPolicy::SEQUENTIAL_1_1,
            2 => CpuPolicy::SEQUENTIAL_2_1,
            4 => CpuPolicy::SEQUENTIAL_4_1,
            8 => CpuPolicy::SEQUENTIAL_8_1,
            _ => unreachable!("closed candidate threads"),
        }
    }

    fn affinity(self) -> &'static str {
        match self.budget {
            1 => "0",
            2 => "0,2",
            4 => "0,2,4,6",
            8 => "0,2,4,6,8,10,12,14",
            _ => unreachable!("closed candidate budget"),
        }
    }
}

const ALL_CANDIDATES: [Candidate; 10] = [
    Candidate {
        budget: 1,
        workers: 1,
        threads: 1,
    },
    Candidate {
        budget: 2,
        workers: 1,
        threads: 2,
    },
    Candidate {
        budget: 2,
        workers: 2,
        threads: 1,
    },
    Candidate {
        budget: 4,
        workers: 1,
        threads: 4,
    },
    Candidate {
        budget: 4,
        workers: 2,
        threads: 2,
    },
    Candidate {
        budget: 4,
        workers: 4,
        threads: 1,
    },
    Candidate {
        budget: 8,
        workers: 1,
        threads: 8,
    },
    Candidate {
        budget: 8,
        workers: 2,
        threads: 4,
    },
    Candidate {
        budget: 8,
        workers: 4,
        threads: 2,
    },
    Candidate {
        budget: 8,
        workers: 8,
        threads: 1,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CandidateAggregate {
    candidate: Candidate,
    rounds: usize,
    exact: bool,
    operational: bool,
    lone_p50_ns: u128,
    batch_elapsed_ns: u128,
    concurrent_p95_ns: u128,
    rss_kib: u64,
    minor_faults: u64,
    major_faults: u64,
}

#[derive(Clone, Copy)]
struct RoundSummary {
    round: usize,
    exact: bool,
    operational: bool,
    lone_p50_ns: u128,
    batch_elapsed_ns: u128,
    concurrent_p95_ns: u128,
    rss_kib: u64,
    minor_faults: u64,
    major_faults: u64,
}

fn aggregate_rounds(candidate: Candidate, rounds: &[RoundSummary]) -> Option<CandidateAggregate> {
    let mut round_numbers: Vec<_> = rounds.iter().map(|round| round.round).collect();
    round_numbers.sort_unstable();
    if round_numbers != [1, 2, 3] {
        return None;
    }
    Some(CandidateAggregate {
        candidate,
        rounds: rounds.len(),
        exact: rounds.iter().all(|round| round.exact),
        operational: rounds.iter().all(|round| round.operational),
        lone_p50_ns: nearest_rank(
            &rounds
                .iter()
                .map(|round| round.lone_p50_ns)
                .collect::<Vec<_>>(),
            50,
            100,
        )?,
        batch_elapsed_ns: nearest_rank(
            &rounds
                .iter()
                .map(|round| round.batch_elapsed_ns)
                .collect::<Vec<_>>(),
            50,
            100,
        )?,
        concurrent_p95_ns: rounds.iter().map(|round| round.concurrent_p95_ns).max()?,
        rss_kib: rounds.iter().map(|round| round.rss_kib).max()?,
        minor_faults: rounds.iter().map(|round| round.minor_faults).max()?,
        major_faults: rounds.iter().map(|round| round.major_faults).max()?,
    })
}

fn nearest_rank<T: Ord + Copy>(samples: &[T], numerator: usize, denominator: usize) -> Option<T> {
    if samples.is_empty() || numerator == 0 || numerator > denominator || denominator == 0 {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = sorted.len().checked_mul(numerator)?.div_ceil(denominator) - 1;
    sorted.get(rank).copied()
}

fn within_five_percent(leader_elapsed: u128, candidate_elapsed: u128) -> bool {
    100_u128
        .checked_mul(leader_elapsed)
        .zip(95_u128.checked_mul(candidate_elapsed))
        .is_some_and(|(leader, candidate)| leader >= candidate)
}

fn within_lone_latency_guard(fastest: u128, candidate: u128) -> bool {
    candidate
        .checked_mul(100)
        .zip(fastest.checked_mul(125))
        .is_some_and(|(candidate, fastest)| candidate <= fastest)
}

fn select_candidate(candidates: &[CandidateAggregate]) -> Option<Candidate> {
    let eligible: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.rounds == 3
                && candidate.exact
                && candidate.operational
                && candidate.rss_kib <= 1_048_576
        })
        .collect();
    let fastest_lone = eligible
        .iter()
        .map(|candidate| candidate.lone_p50_ns)
        .min()?;
    let eligible: Vec<_> = eligible
        .into_iter()
        .filter(|candidate| within_lone_latency_guard(fastest_lone, candidate.lone_p50_ns))
        .collect();
    let leader = eligible
        .iter()
        .map(|candidate| candidate.batch_elapsed_ns)
        .min()?;
    eligible
        .into_iter()
        .filter(|candidate| within_five_percent(leader, candidate.batch_elapsed_ns))
        .min_by(|left, right| {
            left.concurrent_p95_ns
                .cmp(&right.concurrent_p95_ns)
                .then_with(|| left.rss_kib.cmp(&right.rss_kib))
                .then_with(|| left.candidate.workers.cmp(&right.candidate.workers))
        })
        .map(|candidate| candidate.candidate)
}

fn round_robin_assignments(workers: usize, jobs: usize) -> Vec<Vec<usize>> {
    let mut assignments = vec![Vec::new(); workers];
    for job in 0..jobs {
        assignments[job % workers].push(job);
    }
    assignments
}

fn loaded_sample_overlapped(begin_active: usize, end_active: usize) -> bool {
    begin_active != 0 && end_active != 0
}

#[derive(Serialize)]
struct Percentiles {
    samples: usize,
    p50_ns: u128,
    p95_ns: u128,
    p99_ns: u128,
}

fn percentiles(samples: &[Duration]) -> Percentiles {
    let values: Vec<_> = samples.iter().map(Duration::as_nanos).collect();
    Percentiles {
        samples: values.len(),
        p50_ns: nearest_rank(&values, 50, 100).expect("nonempty samples"),
        p95_ns: nearest_rank(&values, 95, 100).expect("nonempty samples"),
        p99_ns: nearest_rank(&values, 99, 100).expect("nonempty samples"),
    }
}

#[derive(Serialize)]
struct SnvMeasurement {
    state: &'static str,
    batch_size: usize,
    latency: Percentiles,
}

#[derive(Serialize)]
struct Faults {
    minor: u64,
    major: u64,
}

#[derive(Serialize)]
struct AssetIdentities<'a> {
    snv_bundle: &'a str,
    model_bundle: &'a str,
    model_profile: &'a str,
    reference_bundle: &'a str,
    reference_member_sha256: &'a str,
    mask_bytes: u64,
    mask_sha256: &'a str,
    compatibility_manifest_sha256: &'a str,
    compatibility_cases_bytes: u64,
    compatibility_cases_sha256: &'a str,
}

#[derive(Serialize)]
struct RuntimeIdentity<'a> {
    pangopup_engine: &'a str,
    ort_crate: &'a str,
    onnx_runtime: &'a str,
    target_os: &'a str,
    target_arch: &'a str,
    kernel: String,
    cpu: String,
    topology: &'a str,
}

#[derive(Serialize)]
struct RoundMeasurement<'a> {
    schema: &'a str,
    measurement_source_sha256: String,
    candidate: String,
    budget: usize,
    workers: usize,
    threads_per_worker: usize,
    round: usize,
    affinity: String,
    assets: AssetIdentities<'a>,
    runtime: RuntimeIdentity<'a>,
    startup_ns: u128,
    lone_m09: Percentiles,
    concurrent: Percentiles,
    concurrent_batch_elapsed_ns: u128,
    model_batch_throughput_per_second: f64,
    snv: Vec<SnvMeasurement>,
    maximum_rss_kib: u64,
    faults: Faults,
    session_invocations: usize,
    logical_context_evaluations: usize,
    exact: bool,
}

#[derive(Serialize)]
struct QualificationMeasurement<'a> {
    schema: &'a str,
    measurement_source_sha256: String,
    candidate: String,
    budget: usize,
    affinity: String,
    cases: usize,
    records: usize,
    exact: bool,
    assets: AssetIdentities<'a>,
}

#[derive(Clone)]
struct AssetPaths {
    snv: PathBuf,
    model: PathBuf,
    reference: PathBuf,
    mask: PathBuf,
}

impl AssetPaths {
    fn from_environment() -> Self {
        Self {
            snv: required_path("PANGOPUP_SNV_BUNDLE"),
            model: required_path("PANGOPUP_MODEL_BUNDLE"),
            reference: required_path("PANGOPUP_REFERENCE_BUNDLE"),
            mask: required_path("PANGOPUP_MASK_MEMBER"),
        }
    }
}

fn required_path(name: &str) -> PathBuf {
    let value = env::var_os(name).unwrap_or_else(|| panic!("set {name} explicitly"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be an absolute path");
    path
}

fn repository_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).expect("read identity member");
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn measurement_source_sha256() -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(include_bytes!("service_scheduling_measurement.rs"))
    )
}

fn cpu_allow_list() -> String {
    fs::read_to_string("/proc/self/status")
        .expect("read /proc/self/status")
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:").map(str::trim))
        .expect("Cpus_allowed_list")
        .to_owned()
}

fn cpu_name() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .expect("read /proc/cpuinfo")
        .lines()
        .find_map(|line| line.strip_prefix("model name\t: "))
        .expect("CPU model name")
        .to_owned()
}

fn validate_host_topology() {
    assert_eq!(cpu_name(), EXPECTED_CPU, "unexpected CPU identity");
    assert_eq!(
        fs::read_to_string("/sys/devices/system/cpu/online")
            .expect("read online CPUs")
            .trim(),
        EXPECTED_ONLINE_CPUS,
        "unexpected online CPU set"
    );
    for cpu in 0_usize..16 {
        let topology = PathBuf::from(format!("/sys/devices/system/cpu/cpu{cpu}/topology"));
        let core = fs::read_to_string(topology.join("core_id"))
            .expect("read CPU core ID")
            .trim()
            .parse::<usize>()
            .expect("numeric core ID");
        let package = fs::read_to_string(topology.join("physical_package_id"))
            .expect("read CPU package ID")
            .trim()
            .parse::<usize>()
            .expect("numeric package ID");
        let siblings =
            fs::read_to_string(topology.join("thread_siblings_list")).expect("read CPU siblings");
        assert_eq!(package, 0, "CPU {cpu} package");
        assert_eq!(core, cpu / 2, "CPU {cpu} core");
        assert_eq!(
            siblings.trim(),
            format!("{}-{}", cpu / 2 * 2, cpu / 2 * 2 + 1),
            "CPU {cpu} SMT siblings"
        );
    }
}

fn kernel_version() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .expect("read kernel version")
        .trim()
        .to_owned()
}

fn process_status() -> (u64, Faults) {
    let status = fs::read_to_string("/proc/self/status").expect("read process status");
    let rss = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|value| value.split_ascii_whitespace().next())
        .and_then(|value| value.parse().ok())
        .expect("VmHWM");
    let stat = fs::read_to_string("/proc/self/stat").expect("read process stat");
    let after_name = stat.rsplit_once(") ").expect("process stat name").1;
    let fields: Vec<_> = after_name.split_ascii_whitespace().collect();
    let minor = fields[7].parse().expect("minor faults");
    let major = fields[9].parse().expect("major faults");
    (rss, Faults { minor, major })
}

fn compat_f32(bits: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(bits, 16).expect("f32 bits"))
}

fn compat_f64(bits: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(bits, 16).expect("f64 bits"))
}

fn hundredths(dtype: &str, bits: &str, loss: bool) -> u8 {
    let rounded = match dtype {
        "f32" => f64::from((compat_f32(bits) * 100.0_f32).round_ties_even()),
        "f64" => (compat_f64(bits) * 100.0_f64).round_ties_even(),
        _ => panic!("unexpected compatibility dtype"),
    };
    (if loss { -rounded } else { rounded }) as u8
}

fn model_variant(case: &CorpusCase) -> Grch38Variant {
    Grch38Variant::new(
        case.input.contig.parse().expect("model contig"),
        GenomicPosition::new(case.input.position).expect("model position"),
        case.input.reference.clone(),
        case.input.alternate.clone(),
    )
    .expect("model variant")
}

fn assert_model_exact(case: &CorpusCase, result: &pangopup_core::ModelScoreResult) -> usize {
    let records = result.records().expect("expected scored result");
    let expected: Vec<_> = case
        .strands
        .iter()
        .flat_map(|strand| {
            strand.expected.masked.iter().map(|score| {
                (
                    score.gene.as_str(),
                    hundredths(&strand.dtype, &score.gain_bits, false),
                    score.gain_position,
                    hundredths(&strand.dtype, &score.loss_bits, true),
                    score.loss_position,
                )
            })
        })
        .collect();
    assert_eq!(records.len(), expected.len(), "{} records", case.id);
    for (record, expected) in records.iter().zip(expected) {
        assert_eq!(record.gene().to_string(), expected.0, "{} gene", case.id);
        assert_eq!(
            record.score().gain().hundredths(),
            expected.1,
            "{} gain",
            case.id
        );
        assert_eq!(
            record.score().gain_position().get(),
            expected.2,
            "{} gain position",
            case.id
        );
        assert_eq!(
            record.score().loss().hundredths(),
            expected.3,
            "{} loss",
            case.id
        );
        assert_eq!(
            record.score().loss_position().get(),
            expected.4,
            "{} loss position",
            case.id
        );
        assert!(record.warnings().is_empty(), "{} warnings", case.id);
    }
    records.len()
}

fn model_cases() -> Vec<CorpusCase> {
    assert_eq!(
        sha256(&repository_path(
            "tests/fixtures/pangolin-compat-v1/manifest.json"
        )),
        COMPAT_MANIFEST_SHA256
    );
    let cases_path = repository_path("tests/fixtures/pangolin-compat-v1/cases.jsonl");
    assert_eq!(
        fs::metadata(&cases_path)
            .expect("compatibility cases metadata")
            .len(),
        COMPAT_CASES_BYTES,
        "compatibility cases byte length"
    );
    let cases = fs::read(&cases_path).expect("read compatibility cases");
    assert_eq!(
        sha256_bytes(&cases),
        COMPAT_CASES_SHA256,
        "compatibility cases SHA-256"
    );
    String::from_utf8(cases)
        .expect("compatibility cases UTF-8")
        .lines()
        .filter_map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("compatibility JSON");
            (value["kind"] == "model")
                .then(|| serde_json::from_value(value).expect("model compatibility case"))
        })
        .collect()
}

fn selected_concurrent_cases(all: &[CorpusCase]) -> Vec<CorpusCase> {
    let selected: Vec<_> = CONCURRENT_IDS
        .iter()
        .map(|id| {
            all.iter()
                .find(|case| case.id == *id)
                .expect("named case")
                .clone()
        })
        .collect();
    assert_eq!(selected.len(), 8);
    selected
}

fn snv_cases() -> Vec<SnvCase> {
    let requests = fs::read_to_string(repository_path(
        "tests/fixtures/snv-regression/requests.tsv",
    ))
    .expect("read SNV requests");
    let expected = fs::read_to_string(repository_path(
        "tests/fixtures/snv-regression/expected.jsonl",
    ))
    .expect("read SNV expected");
    let mut selected = Vec::with_capacity(100);
    for (request, expected) in requests.lines().skip(1).zip(expected.lines()) {
        let expected: ExpectedSnvLine = serde_json::from_str(expected).expect("expected SNV JSON");
        if expected.status != "found" || expected.records.is_empty() {
            continue;
        }
        let columns: Vec<_> = request.split('\t').collect();
        let variant = columns[3].strip_prefix("GRCh38:").expect("GRCh38 request");
        let parts: Vec<_> = variant.split(':').collect();
        let position =
            GenomicPosition::new(parts[1].parse().expect("SNV position")).expect("position");
        let snv = Grch38Snv::new(
            parts[0].parse().expect("SNV contig"),
            position,
            DnaBase::parse(parts[2]).expect("SNV REF"),
            DnaBase::parse(parts[3]).expect("SNV ALT"),
        )
        .expect("SNV");
        let gene = EnsemblGeneId::from_str(columns[4]).expect("SNV gene");
        let expected = expected
            .records
            .into_iter()
            .map(|record| {
                (
                    EnsemblGeneId::from_str(&record.gene).expect("expected gene"),
                    record.gain_score,
                    record.gain_position,
                    record.loss_score,
                    record.loss_position,
                )
            })
            .collect();
        selected.push(SnvCase {
            snv,
            gene,
            expected,
        });
        if selected.len() == 100 {
            break;
        }
    }
    assert_eq!(selected.len(), 100, "fixture must supply 100 found rows");
    selected
}

fn assert_snv_exact(case: &SnvCase, result: &LookupResult) {
    assert!(result.source_reference_ambiguities().is_empty());
    assert_eq!(result.records().len(), case.expected.len());
    for (record, expected) in result.records().iter().zip(&case.expected) {
        assert_eq!(record.gene(), expected.0);
        assert_eq!(record.score().gain().to_string(), expected.1);
        assert_eq!(record.score().gain_position().get(), expected.2);
        assert_eq!(record.score().loss_text().to_string(), expected.3);
        assert_eq!(record.score().loss_position().get(), expected.4);
    }
}

fn warm_snv_series(provider: &SnvBundleOpen, cases: &[SnvCase]) {
    for case in cases {
        let result = provider
            .lookup(case.snv, Some(case.gene))
            .expect("SNV warmup lookup");
        assert_snv_exact(case, &result);
    }
}

fn measure_snv_series(
    provider: &SnvBundleOpen,
    cases: &[SnvCase],
    state: &'static str,
    active: Option<&AtomicUsize>,
) -> Vec<SnvMeasurement> {
    [1_usize, 10, 100]
        .into_iter()
        .map(|batch_size| {
            let mut samples = Vec::with_capacity(25);
            for _ in 0..25 {
                if let Some(active) = active {
                    assert!(
                        loaded_sample_overlapped(active.load(AtomicOrdering::Acquire), 1),
                        "loaded SNV began idle"
                    );
                }
                let started = Instant::now();
                for case in &cases[..batch_size] {
                    let result = provider
                        .lookup(case.snv, Some(case.gene))
                        .expect("SNV lookup");
                    assert_snv_exact(case, &result);
                }
                samples.push(started.elapsed());
                if let Some(active) = active {
                    assert!(
                        loaded_sample_overlapped(1, active.load(AtomicOrdering::Acquire)),
                        "loaded SNV ended idle"
                    );
                }
            }
            SnvMeasurement {
                state,
                batch_size,
                latency: percentiles(&samples),
            }
        })
        .collect()
}

struct WorkerOutcome {
    ordinal: usize,
    elapsed: Duration,
    records: usize,
    session_invocations: usize,
    logical_context_evaluations: usize,
}

enum WorkerCommand {
    Serial {
        case: CorpusCase,
        result: mpsc::Sender<WorkerOutcome>,
    },
    Concurrent {
        jobs: Vec<(usize, CorpusCase)>,
        barrier: Arc<Barrier>,
        release: Arc<OnceLock<Instant>>,
        active: Arc<AtomicUsize>,
        result: mpsc::Sender<WorkerOutcome>,
    },
    Qualify {
        cases: Vec<CorpusCase>,
        result: mpsc::Sender<(usize, usize)>,
    },
    Stop,
}

struct Worker {
    command: mpsc::Sender<WorkerCommand>,
    join: thread::JoinHandle<()>,
}

fn spawn_worker(paths: AssetPaths, policy: CpuPolicy) -> (Worker, Duration) {
    let (command_tx, command_rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel(0);
    let join = thread::spawn(move || {
        let started = Instant::now();
        let model =
            ModelKernel::open_with_cpu_policy(&paths.model, policy).expect("production model");
        assert_eq!(model.bundle_identity().as_str(), MODEL_ID);
        assert_eq!(model.profile(), MODEL_PROFILE);
        let reference =
            ReferenceBundleOpen::open_identified(&paths.reference).expect("production reference");
        assert_eq!(reference.provenance().bundle_id(), REFERENCE_ID);
        assert_eq!(reference.identity().sha256(), REFERENCE_MEMBER);
        let mask = MaskDomainsOpen::open_identified(&paths.mask).expect("production mask");
        assert_eq!(mask.identity().bytes(), MASK_BYTES);
        assert_eq!(mask.identity().sha256(), MASK_SHA256);
        let mut scorer = VariantScorer::new(reference, mask, model);
        ready_tx.send(started.elapsed()).expect("worker ready");
        while let Ok(command) = command_rx.recv() {
            match command {
                WorkerCommand::Serial { case, result } => {
                    let started = Instant::now();
                    let scored = scorer.score(&model_variant(&case)).expect("serial score");
                    let elapsed = started.elapsed();
                    let records = assert_model_exact(&case, &scored);
                    let accounting = scorer.last_model_accounting();
                    result
                        .send(WorkerOutcome {
                            ordinal: 0,
                            elapsed,
                            records,
                            session_invocations: accounting.session_invocations,
                            logical_context_evaluations: accounting.logical_context_evaluations,
                        })
                        .expect("serial outcome");
                }
                WorkerCommand::Concurrent {
                    jobs,
                    barrier,
                    release,
                    active,
                    result,
                } => {
                    active.fetch_add(1, AtomicOrdering::AcqRel);
                    barrier.wait();
                    let common_start = loop {
                        if let Some(started) = release.get() {
                            break *started;
                        }
                        std::hint::spin_loop();
                    };
                    for (ordinal, case) in jobs {
                        let scored = scorer
                            .score(&model_variant(&case))
                            .expect("concurrent score");
                        let records = assert_model_exact(&case, &scored);
                        let accounting = scorer.last_model_accounting();
                        result
                            .send(WorkerOutcome {
                                ordinal,
                                elapsed: common_start.elapsed(),
                                records,
                                session_invocations: accounting.session_invocations,
                                logical_context_evaluations: accounting.logical_context_evaluations,
                            })
                            .expect("concurrent outcome");
                    }
                    active.fetch_sub(1, AtomicOrdering::AcqRel);
                }
                WorkerCommand::Qualify { cases, result } => {
                    let mut records = 0;
                    for case in &cases {
                        let scored = scorer
                            .score(&model_variant(case))
                            .expect("qualification score");
                        records += assert_model_exact(case, &scored);
                    }
                    result
                        .send((cases.len(), records))
                        .expect("qualification outcome");
                }
                WorkerCommand::Stop => break,
            }
        }
    });
    let open = ready_rx.recv().expect("worker initialization");
    (
        Worker {
            command: command_tx,
            join,
        },
        open,
    )
}

fn stop_workers(workers: Vec<Worker>) {
    for worker in &workers {
        worker
            .command
            .send(WorkerCommand::Stop)
            .expect("stop worker");
    }
    for worker in workers {
        worker.join.join().expect("join worker");
    }
}

fn validate_snv(paths: &AssetPaths) -> SnvBundleOpen {
    let provider = SnvBundleOpen::open(&paths.snv).expect("production SNV bundle");
    assert_eq!(provider.bundle_id(), SNV_ID);
    provider
}

fn validate_inputs(candidate: Candidate, round: usize) {
    assert!((1..=3).contains(&round), "round must be 1, 2, or 3");
    assert_eq!(
        cpu_allow_list(),
        candidate.affinity(),
        "unexpected physical-core affinity"
    );
    assert_eq!(env::consts::OS, "linux");
    assert_eq!(env::consts::ARCH, "x86_64");
    validate_host_topology();
}

#[test]
#[ignore = "coordinator-only retained-production scheduling measurement"]
fn retained_production_service_scheduling_round() {
    let candidate =
        Candidate::parse(&env::var("PANGOPUP_SCHEDULING_CANDIDATE").expect("set candidate"))
            .expect("closed candidate");
    let round = env::var("PANGOPUP_SCHEDULING_ROUND")
        .expect("set round")
        .parse::<usize>()
        .expect("numeric round");
    validate_inputs(candidate, round);
    let paths = AssetPaths::from_environment();
    let snv = validate_snv(&paths);
    let snv_cases = snv_cases();
    warm_snv_series(&snv, &snv_cases);
    let all_cases = model_cases();
    assert_eq!(all_cases.len(), 14);
    let concurrent_cases = selected_concurrent_cases(&all_cases);
    let m09 = all_cases
        .iter()
        .find(|case| case.id.starts_with("M09-"))
        .expect("M09")
        .clone();

    let startup_started = Instant::now();
    let mut workers = Vec::with_capacity(candidate.workers);
    for _ in 0..candidate.workers {
        let (worker, _) = spawn_worker(paths.clone(), candidate.policy());
        workers.push(worker);
    }
    let startup_ns = startup_started.elapsed().as_nanos();

    for worker in &workers {
        let (tx, rx) = mpsc::channel();
        worker
            .command
            .send(WorkerCommand::Serial {
                case: m09.clone(),
                result: tx,
            })
            .expect("warm worker");
        rx.recv().expect("warm result");
    }
    let mut session_invocations = 0;
    let mut logical_context_evaluations = 0;
    let mut lone = Vec::with_capacity(3);
    for _ in 0..3 {
        let (tx, rx) = mpsc::channel();
        workers[0]
            .command
            .send(WorkerCommand::Serial {
                case: m09.clone(),
                result: tx,
            })
            .expect("serial request");
        let outcome = rx.recv().expect("serial result");
        lone.push(outcome.elapsed);
        session_invocations += outcome.session_invocations;
        logical_context_evaluations += outcome.logical_context_evaluations;
    }

    let mut snv_measurements = measure_snv_series(&snv, &snv_cases, "idle", None);
    let assignments = round_robin_assignments(candidate.workers, concurrent_cases.len());
    let barrier = Arc::new(Barrier::new(candidate.workers + 1));
    let release = Arc::new(OnceLock::new());
    let active = Arc::new(AtomicUsize::new(0));
    let (result_tx, result_rx) = mpsc::channel();
    for (worker, jobs) in workers.iter().zip(assignments) {
        let jobs = jobs
            .into_iter()
            .map(|index| (index, concurrent_cases[index].clone()))
            .collect();
        worker
            .command
            .send(WorkerCommand::Concurrent {
                jobs,
                barrier: Arc::clone(&barrier),
                release: Arc::clone(&release),
                active: Arc::clone(&active),
                result: result_tx.clone(),
            })
            .expect("concurrent work");
    }
    barrier.wait();
    assert_eq!(active.load(AtomicOrdering::Acquire), candidate.workers);
    let common_start = Instant::now();
    release.set(common_start).expect("single release");
    snv_measurements.extend(measure_snv_series(
        &snv,
        &snv_cases,
        "loaded",
        Some(&active),
    ));
    let mut outcomes = Vec::with_capacity(8);
    for _ in 0..8 {
        outcomes.push(result_rx.recv().expect("concurrent result"));
    }
    let batch_elapsed = common_start.elapsed();
    outcomes.sort_by_key(|outcome| outcome.ordinal);
    assert_eq!(
        outcomes
            .iter()
            .map(|outcome| outcome.ordinal)
            .collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
    assert!(outcomes.iter().all(|outcome| outcome.records > 0));
    session_invocations += outcomes
        .iter()
        .map(|outcome| outcome.session_invocations)
        .sum::<usize>();
    logical_context_evaluations += outcomes
        .iter()
        .map(|outcome| outcome.logical_context_evaluations)
        .sum::<usize>();
    let concurrent_latencies: Vec<_> = outcomes.iter().map(|outcome| outcome.elapsed).collect();
    while active.load(AtomicOrdering::Acquire) != 0 {
        thread::yield_now();
    }
    assert_eq!(active.load(AtomicOrdering::Acquire), 0);
    let (maximum_rss_kib, faults) = process_status();
    stop_workers(workers);

    let output = RoundMeasurement {
        schema: "pangopup-service-scheduling-round-v1",
        measurement_source_sha256: measurement_source_sha256(),
        candidate: candidate.name(),
        budget: candidate.budget,
        workers: candidate.workers,
        threads_per_worker: candidate.threads,
        round,
        affinity: cpu_allow_list(),
        assets: AssetIdentities {
            snv_bundle: SNV_ID,
            model_bundle: MODEL_ID,
            model_profile: MODEL_PROFILE,
            reference_bundle: REFERENCE_ID,
            reference_member_sha256: REFERENCE_MEMBER,
            mask_bytes: MASK_BYTES,
            mask_sha256: MASK_SHA256,
            compatibility_manifest_sha256: COMPAT_MANIFEST_SHA256,
            compatibility_cases_bytes: COMPAT_CASES_BYTES,
            compatibility_cases_sha256: COMPAT_CASES_SHA256,
        },
        runtime: RuntimeIdentity {
            pangopup_engine: env!("CARGO_PKG_VERSION"),
            ort_crate: ORT_CRATE_VERSION,
            onnx_runtime: ONNX_RUNTIME_VERSION,
            target_os: env::consts::OS,
            target_arch: env::consts::ARCH,
            kernel: kernel_version(),
            cpu: cpu_name(),
            topology: EXPECTED_TOPOLOGY,
        },
        startup_ns,
        lone_m09: percentiles(&lone),
        concurrent: percentiles(&concurrent_latencies),
        concurrent_batch_elapsed_ns: batch_elapsed.as_nanos(),
        model_batch_throughput_per_second: 8.0 / batch_elapsed.as_secs_f64(),
        snv: snv_measurements,
        maximum_rss_kib,
        faults,
        session_invocations,
        logical_context_evaluations,
        exact: true,
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("measurement JSON")
    );
}

#[test]
#[ignore = "coordinator-only selected mapping full compatibility rerun"]
fn retained_production_selected_mapping_qualification() {
    let candidate =
        Candidate::parse(&env::var("PANGOPUP_SCHEDULING_CANDIDATE").expect("set candidate"))
            .expect("closed candidate");
    validate_inputs(candidate, 1);
    let paths = AssetPaths::from_environment();
    let _snv = validate_snv(&paths);
    let cases = model_cases();
    assert_eq!(cases.len(), 14);
    let mut workers = Vec::with_capacity(candidate.workers);
    for _ in 0..candidate.workers {
        workers.push(spawn_worker(paths.clone(), candidate.policy()).0);
    }
    let assignments = round_robin_assignments(candidate.workers, cases.len());
    let (tx, rx) = mpsc::channel();
    for (worker, assigned) in workers.iter().zip(assignments) {
        let cases = assigned
            .into_iter()
            .map(|index| cases[index].clone())
            .collect();
        worker
            .command
            .send(WorkerCommand::Qualify {
                cases,
                result: tx.clone(),
            })
            .expect("qualify work");
    }
    let mut case_count = 0;
    let mut record_count = 0;
    for _ in 0..candidate.workers {
        let (cases, records) = rx.recv().expect("qualification result");
        case_count += cases;
        record_count += records;
    }
    stop_workers(workers);
    assert_eq!(case_count, 14);
    assert_eq!(record_count, 21);
    let output = QualificationMeasurement {
        schema: "pangopup-service-scheduling-qualification-v1",
        measurement_source_sha256: measurement_source_sha256(),
        candidate: candidate.name(),
        budget: candidate.budget,
        affinity: cpu_allow_list(),
        cases: case_count,
        records: record_count,
        exact: true,
        assets: AssetIdentities {
            snv_bundle: SNV_ID,
            model_bundle: MODEL_ID,
            model_profile: MODEL_PROFILE,
            reference_bundle: REFERENCE_ID,
            reference_member_sha256: REFERENCE_MEMBER,
            mask_bytes: MASK_BYTES,
            mask_sha256: MASK_SHA256,
            compatibility_manifest_sha256: COMPAT_MANIFEST_SHA256,
            compatibility_cases_bytes: COMPAT_CASES_BYTES,
            compatibility_cases_sha256: COMPAT_CASES_SHA256,
        },
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("qualification JSON")
    );
}

#[test]
fn nearest_rank_and_round_aggregation_are_exact() {
    assert_eq!(nearest_rank(&[9, 1, 5], 50, 100), Some(5));
    assert_eq!(nearest_rank(&[9, 1, 5], 95, 100), Some(9));
    assert_eq!(nearest_rank::<u64>(&[], 50, 100), None);
    let candidate = ALL_CANDIDATES[0];
    let rounds = [
        RoundSummary {
            round: 1,
            exact: true,
            operational: true,
            lone_p50_ns: 9,
            batch_elapsed_ns: 90,
            concurrent_p95_ns: 900,
            rss_kib: 9,
            minor_faults: 90,
            major_faults: 0,
        },
        RoundSummary {
            round: 2,
            exact: true,
            operational: true,
            lone_p50_ns: 1,
            batch_elapsed_ns: 10,
            concurrent_p95_ns: 100,
            rss_kib: 1,
            minor_faults: 10,
            major_faults: 1,
        },
        RoundSummary {
            round: 3,
            exact: true,
            operational: true,
            lone_p50_ns: 5,
            batch_elapsed_ns: 50,
            concurrent_p95_ns: 500,
            rss_kib: 5,
            minor_faults: 50,
            major_faults: 0,
        },
    ];
    let aggregate = aggregate_rounds(candidate, &rounds).expect("complete rounds");
    assert_eq!(aggregate.lone_p50_ns, 5);
    assert_eq!(aggregate.batch_elapsed_ns, 50);
    assert_eq!(aggregate.concurrent_p95_ns, 900);
    assert_eq!(aggregate.rss_kib, 9);
    assert_eq!(aggregate.minor_faults, 90);
    assert_eq!(aggregate.major_faults, 1);
    assert!(aggregate.exact && aggregate.operational);
    assert!(aggregate_rounds(candidate, &rounds[..2]).is_none());
}

#[test]
fn five_percent_comparison_uses_exact_checked_arithmetic() {
    assert!(within_five_percent(95, 100));
    assert!(!within_five_percent(94, 100));
    assert!(!within_five_percent(u128::MAX, u128::MAX));
    assert!(within_lone_latency_guard(100, 125));
    assert!(!within_lone_latency_guard(100, 126));
    assert!(!within_lone_latency_guard(u128::MAX, u128::MAX));
}

#[test]
fn selection_applies_latency_rss_and_ties_mechanically() {
    let base = |workers, threads, lone, batch, p95, rss| CandidateAggregate {
        candidate: Candidate {
            budget: 4,
            workers,
            threads,
        },
        rounds: 3,
        exact: true,
        operational: true,
        lone_p50_ns: lone,
        batch_elapsed_ns: batch,
        concurrent_p95_ns: p95,
        rss_kib: rss,
        minor_faults: 1,
        major_faults: 0,
    };
    let candidates = [
        base(1, 4, 100, 100, 90, 100),
        base(2, 2, 120, 104, 80, 200),
        base(4, 1, 126, 90, 10, 50),
    ];
    assert_eq!(
        select_candidate(&candidates),
        Some(Candidate {
            budget: 4,
            workers: 2,
            threads: 2
        })
    );
}

#[test]
fn selection_rejects_incomplete_inexact_failed_and_oversized_sets() {
    let candidate = CandidateAggregate {
        candidate: ALL_CANDIDATES[0],
        rounds: 2,
        exact: true,
        operational: true,
        lone_p50_ns: 1,
        batch_elapsed_ns: 1,
        concurrent_p95_ns: 1,
        rss_kib: 1,
        minor_faults: 1,
        major_faults: 0,
    };
    assert_eq!(select_candidate(&[candidate]), None);
    for rejected in [
        CandidateAggregate {
            rounds: 3,
            exact: false,
            ..candidate
        },
        CandidateAggregate {
            rounds: 3,
            operational: false,
            ..candidate
        },
        CandidateAggregate {
            rounds: 3,
            rss_kib: 1_048_577,
            ..candidate
        },
    ] {
        assert_eq!(select_candidate(&[rejected]), None);
    }
}

#[test]
fn round_robin_is_stable_and_overlap_rejection_is_explicit() {
    assert_eq!(
        round_robin_assignments(3, 8),
        vec![vec![0, 3, 6], vec![1, 4, 7], vec![2, 5]]
    );
    assert!(loaded_sample_overlapped(1, 1));
    assert!(!loaded_sample_overlapped(0, 1));
    assert!(!loaded_sample_overlapped(1, 0));
}

#[test]
fn closed_candidate_parser_rejects_invalid_counts() {
    assert_eq!(
        Candidate::parse("2x2"),
        Ok(Candidate {
            budget: 4,
            workers: 2,
            threads: 2
        })
    );
    assert!(Candidate::parse("3x1").is_err());
    assert!(Candidate::parse("1x3").is_err());
}

#[test]
fn compatibility_member_and_measurement_source_identities_are_exact() {
    assert_eq!(model_cases().len(), 14);
    assert_eq!(
        measurement_source_sha256(),
        sha256(&repository_path(
            "crates/pangopup-engine/tests/service_scheduling_measurement.rs"
        ))
    );
}
