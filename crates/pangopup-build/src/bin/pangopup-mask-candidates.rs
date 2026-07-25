//! Private, feature-gated Ticket 012 GENCODE mask qualification CLI.

use pangopup_build::mask::{
    BENCHMARK_PERMUTATIONS, BUILDER_SOURCE_SHA256, BenchmarkHost, BenchmarkMethod,
    BenchmarkResources, BenchmarkRunInput, CandidateMeasurement, CaptureArguments,
    EnvironmentPolicy, GTF_BYTES, GTF_SHA256, Identity, MASK_PROFILE, MaskBenchmarkReport,
    MaskBuildError, PINNED_MASK_ZSTANDARD, QueryOutcome, REPORT_SCHEMA, RoundMeasurement,
    benchmark_phase, capture_phase, evaluate_mask_candidates, inspect_phase,
    load_compatibility_points, open_candidate_for_benchmark, plan_capture_promotion, prepare_phase,
    promote_sealed_capture, query_prepared_candidate, request_cancellation, reuse_sealed_phases,
};
use pangopup_core::{EnsemblGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask_candidates::{MaskCandidateCodec, MaskCandidateReader, MaskQueryBuffer};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::BTreeMap,
    fs::File,
    hint::black_box,
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

const HELP: &str = "usage: pangopup-mask-candidates <command> [options]\n\
commands:\n\
  capture --database ABS --gtf ABS --python ABS --python-bytes N --python-sha256 HEX --python-launcher ABS --python-launcher-link-bytes N --python-launcher-link-sha256 HEX --pyvenv-config-bytes N --pyvenv-config-sha256 HEX --output-parent ABS\n\
  prepare --stage ABS --compatibility-corpus ABS\n\
  inspect --stage ABS\n\
  query --stage ABS --candidate interval-tree|domains|binned-postings --contig CONTIG --position N [--gene ENSG...]\n\
  benchmark --stage ABS\n\
  reuse --prior-stage ABS --output-parent ABS --authorization ABS\n\
  plan-capture-promotion --prior-stage ABS --source-builder-sha256 HEX\n\
  promote-capture --prior-stage ABS --output-parent ABS --authorization ABS\n\
all successful commands emit one canonical JSON line; failures emit one sanitized JSON line";

struct CountingAllocator;
static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static TRACK_OPEN: AtomicBool = AtomicBool::new(false);
static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_BYTES: AtomicU64 = AtomicU64::new(0);
static CURRENT_BYTES: AtomicU64 = AtomicU64::new(0);
static OPEN_BASELINE: AtomicU64 = AtomicU64::new(0);
static OPEN_PEAK: AtomicU64 = AtomicU64::new(0);
static CANCELLED: AtomicBool = AtomicBool::new(false);

fn allocation_added(size: usize) {
    let size = size as u64;
    let current = CURRENT_BYTES.fetch_add(size, Ordering::SeqCst) + size;
    if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOCATION_BYTES.fetch_add(size, Ordering::Relaxed);
    }
    if TRACK_OPEN.load(Ordering::Relaxed) {
        OPEN_PEAK.fetch_max(current, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the allocation is delegated unchanged to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocation_added(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT_BYTES.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: pointer and layout are the original allocation pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the original pair and requested new size are delegated unchanged.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                allocation_added(new_size - layout.size());
            } else {
                CURRENT_BYTES.fetch_sub((layout.size() - new_size) as u64, Ordering::SeqCst);
            }
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Serialize)]
struct ErrorOutcome<'a> {
    ok: bool,
    command: &'a str,
    code: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct HelpOutcome<'a> {
    ok: bool,
    command: &'a str,
    help: &'a str,
}

#[derive(Serialize)]
struct TraceRecord<'a> {
    ordinal: u16,
    metadata_pages: &'a [u64],
    payload_pages: &'a [u64],
}

#[derive(Serialize)]
struct QueryValue {
    plus: Vec<QueryGene>,
    minus: Vec<QueryGene>,
}

#[derive(Serialize)]
struct QueryGene {
    id: String,
    boundaries: Vec<u32>,
}

#[derive(Clone, Copy)]
struct ResourceUsage {
    maximum_rss_bytes: u64,
    minor_faults: u64,
    major_faults: u64,
}

fn main() {
    install_cancellation_handlers();
    let mut arguments = std::env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "--help".into());
    let rest = arguments.collect::<Vec<_>>();
    let result = match command.as_str() {
        "help" | "--help" | "-h" if rest.is_empty() => emit(&HelpOutcome {
            ok: true,
            command: "help",
            help: HELP,
        }),
        "capture" => command_capture(&rest),
        "prepare" => command_prepare(&rest),
        "inspect" => command_inspect(&rest),
        "query" => command_query(&rest),
        "benchmark" => command_benchmark(&rest),
        "reuse" => command_reuse(&rest),
        "plan-capture-promotion" => command_plan_capture_promotion(&rest),
        "promote-capture" => command_promote_capture(&rest),
        _ => Err(MaskBuildError::new("USAGE", "unknown command or arguments")),
    };
    if let Err(error) = result {
        let _ = emit(&ErrorOutcome {
            ok: false,
            command: command_name(&command),
            code: error.code(),
            message: error.message(),
        });
        std::process::exit(2);
    }
}

fn command_name(value: &str) -> &str {
    match value {
        "capture"
        | "prepare"
        | "inspect"
        | "query"
        | "benchmark"
        | "reuse"
        | "plan-capture-promotion"
        | "promote-capture" => value,
        _ => "unknown",
    }
}

fn emit(value: &impl Serialize) -> Result<(), MaskBuildError> {
    let encoded = serde_jcs::to_string(value)
        .map_err(|_| MaskBuildError::new("JSON", "result encoding failed"))?;
    println!("{encoded}");
    Ok(())
}

fn options(
    arguments: &[String],
    required: &[&str],
    optional: &[&str],
) -> Result<BTreeMap<String, String>, MaskBuildError> {
    if !arguments.len().is_multiple_of(2) {
        return Err(MaskBuildError::new("USAGE", "option value is missing"));
    }
    let allowed = required.iter().chain(optional).copied().collect::<Vec<_>>();
    let mut result = BTreeMap::new();
    for pair in arguments.chunks_exact(2) {
        if !allowed.contains(&pair[0].as_str())
            || pair[1].is_empty()
            || result.insert(pair[0].clone(), pair[1].clone()).is_some()
        {
            return Err(MaskBuildError::new(
                "USAGE",
                "unknown, empty, or duplicate option",
            ));
        }
    }
    if required.iter().any(|name| !result.contains_key(*name)) {
        return Err(MaskBuildError::new("USAGE", "required option is missing"));
    }
    Ok(result)
}

fn required_path(values: &BTreeMap<String, String>, name: &str) -> PathBuf {
    PathBuf::from(&values[name])
}

fn parse_u64(value: &str, label: &'static str) -> Result<u64, MaskBuildError> {
    value
        .parse()
        .map_err(|_| MaskBuildError::new("USAGE", format!("{label} is invalid")))
}

fn command_capture(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(
        arguments,
        &[
            "--database",
            "--gtf",
            "--python",
            "--python-bytes",
            "--python-sha256",
            "--python-launcher",
            "--python-launcher-link-bytes",
            "--python-launcher-link-sha256",
            "--pyvenv-config-bytes",
            "--pyvenv-config-sha256",
            "--output-parent",
        ],
        &[],
    )?;
    let outcome = capture_phase(&CaptureArguments {
        database: required_path(&values, "--database"),
        gtf: required_path(&values, "--gtf"),
        python: required_path(&values, "--python"),
        python_launcher: required_path(&values, "--python-launcher"),
        output_parent: required_path(&values, "--output-parent"),
        expected_database: Identity {
            bytes: pangopup_build::mask::DATABASE_BYTES,
            sha256: pangopup_build::mask::DATABASE_SHA256.into(),
        },
        expected_gtf: Identity {
            bytes: GTF_BYTES,
            sha256: GTF_SHA256.into(),
        },
        expected_python: Some(Identity {
            bytes: parse_u64(&values["--python-bytes"], "Python byte count")?,
            sha256: values["--python-sha256"].clone(),
        }),
        expected_launcher_link: Identity {
            bytes: parse_u64(
                &values["--python-launcher-link-bytes"],
                "Python launcher link byte count",
            )?,
            sha256: values["--python-launcher-link-sha256"].clone(),
        },
        expected_pyvenv_config: Identity {
            bytes: parse_u64(&values["--pyvenv-config-bytes"], "pyvenv config byte count")?,
            sha256: values["--pyvenv-config-sha256"].clone(),
        },
        environment_policy: EnvironmentPolicy::production(),
    })?;
    emit(&outcome)
}

fn command_prepare(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(arguments, &["--stage", "--compatibility-corpus"], &[])?;
    let points = load_compatibility_points(&required_path(&values, "--compatibility-corpus"))?;
    let outcome = prepare_phase(&required_path(&values, "--stage"), &points)?;
    emit(&outcome)
}

fn command_inspect(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(arguments, &["--stage"], &[])?;
    emit(&inspect_phase(&required_path(&values, "--stage"))?)
}

fn codec(value: &str) -> Result<MaskCandidateCodec, MaskBuildError> {
    MaskCandidateCodec::ALL
        .into_iter()
        .find(|codec| codec.name() == value)
        .ok_or_else(|| MaskBuildError::new("USAGE", "candidate codec is invalid"))
}

fn command_query(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(
        arguments,
        &["--stage", "--candidate", "--contig", "--position"],
        &["--gene"],
    )?;
    let contig = Grch38Contig::from_str(&values["--contig"])
        .map_err(|_| MaskBuildError::new("USAGE", "contig is invalid"))?;
    let position = values["--position"]
        .parse::<u32>()
        .ok()
        .and_then(|value| GenomicPosition::new(value).ok())
        .ok_or_else(|| MaskBuildError::new("USAGE", "position is invalid"))?;
    let stable = values
        .get("--gene")
        .map(|value| {
            EnsemblGeneId::from_str(value)
                .map_err(|_| MaskBuildError::new("USAGE", "stable gene filter is invalid"))
        })
        .transpose()?;
    let outcome: QueryOutcome = query_prepared_candidate(
        &required_path(&values, "--stage"),
        codec(&values["--candidate"])?,
        contig,
        position,
        stable,
    )?;
    emit(&outcome)
}

fn command_benchmark(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(arguments, &["--stage"], &[])?;
    let outcome = benchmark_phase(&required_path(&values, "--stage"), run_benchmark)?;
    emit(&outcome)
}

fn command_reuse(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(
        arguments,
        &["--prior-stage", "--output-parent", "--authorization"],
        &[],
    )?;
    emit(&reuse_sealed_phases(
        &required_path(&values, "--prior-stage"),
        &required_path(&values, "--output-parent"),
        &required_path(&values, "--authorization"),
    )?)
}

fn command_plan_capture_promotion(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(
        arguments,
        &["--prior-stage", "--source-builder-sha256"],
        &[],
    )?;
    emit(&plan_capture_promotion(
        &required_path(&values, "--prior-stage"),
        &values["--source-builder-sha256"],
    )?)
}

fn command_promote_capture(arguments: &[String]) -> Result<(), MaskBuildError> {
    let values = options(
        arguments,
        &["--prior-stage", "--output-parent", "--authorization"],
        &[],
    )?;
    emit(&promote_sealed_capture(
        &required_path(&values, "--prior-stage"),
        &required_path(&values, "--output-parent"),
        &required_path(&values, "--authorization"),
    )?)
}

fn run_benchmark(input: &BenchmarkRunInput) -> Result<MaskBenchmarkReport, MaskBuildError> {
    if env!("PANGOPUP_BUILD_PROFILE") != "release" {
        return Err(MaskBuildError::new(
            "BENCHMARK_PROFILE",
            "qualification benchmark requires an optimized release binary",
        ));
    }
    let executable = executable_identity()?;
    let host = pin_and_describe_host(executable.clone())?;
    let resources_before = resource_usage()?;
    let preflight = preflight_queries(input)?;
    let mut by_codec: BTreeMap<MaskCandidateCodec, Vec<RoundMeasurement>> = MaskCandidateCodec::ALL
        .into_iter()
        .map(|codec| (codec, Vec::with_capacity(6)))
        .collect();

    for (round_index, permutation) in BENCHMARK_PERMUTATIONS.iter().enumerate() {
        cancelled()?;
        let mut readers = BTreeMap::new();
        let mut open_metrics = BTreeMap::new();
        for codec in permutation {
            let candidate = candidate_input(input, *codec)?;
            let authenticated = open_candidate_for_benchmark(candidate)?;
            begin_open_tracking();
            let started = Instant::now();
            let reader_result = MaskCandidateReader::open_held(authenticated.reader_file()?);
            let open_ns = elapsed_ns(started)?;
            let peak = end_open_tracking();
            let reader = reader_result?;
            if reader.codec() != *codec || reader.file_len() != candidate.identity.bytes {
                return Err(MaskBuildError::new(
                    "BENCHMARK_EVIDENCE",
                    "opened candidate identity drifted",
                ));
            }
            readers.insert(*codec, (reader, authenticated));
            open_metrics.insert(*codec, (open_ns, peak));
        }

        for (schedule_position, codec) in permutation.iter().enumerate() {
            cancelled()?;
            let reader = &readers
                .get(codec)
                .ok_or_else(|| MaskBuildError::new("BENCHMARK", "reader is missing"))?
                .0;
            let mut output = MaskQueryBuffer::with_capacity(
                preflight.gene_capacity,
                preflight.boundary_capacity,
            );
            let mut timings = vec![0_u64; 100_000];
            for operation in 0..10_000_usize {
                if operation % 1_000 == 0 {
                    cancelled()?;
                }
                let query = preflight.queries[operation % 1_000];
                point_query(reader, query, &mut output)?;
                black_box(query_checksum(&output));
            }
            let before = resource_usage()?;
            reset_allocation_tracking();
            let tracking = AllocationTracking::begin();
            for (operation, timing) in timings.iter_mut().enumerate() {
                if operation % 1_000 == 0 {
                    cancelled()?;
                }
                let query = preflight.queries[operation % 1_000];
                let started = Instant::now();
                point_query(reader, query, &mut output)?;
                *timing = elapsed_ns(started)?;
                black_box(query_checksum(&output));
            }
            drop(tracking);
            let allocation_calls = ALLOCATION_CALLS.load(Ordering::SeqCst);
            let allocation_bytes = ALLOCATION_BYTES.load(Ordering::SeqCst);
            let after = resource_usage()?;
            timings.sort_unstable();
            let (open_ns, open_peak_heap_bytes) = open_metrics[codec];
            by_codec
                .get_mut(codec)
                .expect("fixed codec map")
                .push(RoundMeasurement {
                    round: round_index as u8,
                    schedule_position: schedule_position as u8,
                    p50_ns: timings[49_999],
                    p95_ns: timings[94_999],
                    open_ns,
                    open_peak_heap_bytes,
                    warmed_allocation_calls: allocation_calls,
                    warmed_allocation_bytes: allocation_bytes,
                    maximum_rss_bytes: after.maximum_rss_bytes,
                    minor_faults: after.minor_faults.saturating_sub(before.minor_faults),
                    major_faults: after.major_faults.saturating_sub(before.major_faults),
                });
        }
        for (_, authenticated) in readers.values_mut() {
            authenticated.reauthenticate()?;
        }
        drop(readers);
    }

    let mut candidates = Vec::with_capacity(3);
    for codec in MaskCandidateCodec::ALL {
        let input_candidate = candidate_input(input, codec)?;
        let rounds = by_codec.remove(&codec).expect("fixed codec map");
        let mut p50 = rounds.iter().map(|round| round.p50_ns).collect::<Vec<_>>();
        let mut p95 = rounds.iter().map(|round| round.p95_ns).collect::<Vec<_>>();
        p50.sort_unstable();
        p95.sort_unstable();
        let pages = preflight
            .pages
            .get(&codec)
            .ok_or_else(|| MaskBuildError::new("PAGE_TRACE", "page evidence is missing"))?;
        let mut authenticated = open_candidate_for_benchmark(input_candidate)?;
        let compressed = zstd_size(authenticated.file(), input_candidate.identity.bytes)?;
        authenticated.reauthenticate()?;
        let allocation_contract = rounds
            .iter()
            .all(|round| round.warmed_allocation_calls == 0 && round.warmed_allocation_bytes == 0);
        candidates.push(CandidateMeasurement {
            codec,
            member: input_candidate.identity.clone(),
            pinned_zstandard_bytes: compressed,
            pinned_zstandard: PINNED_MASK_ZSTANDARD.into(),
            semantic_certified: true,
            corruption_controls_passed: true,
            allocation_contract_passed: allocation_contract,
            page_trace_sha256: pages.sha256.clone(),
            metadata_pages: pages.metadata_pages,
            median_payload_pages: pages.median_payload_pages,
            p95_payload_pages: pages.p95_payload_pages,
            headline_p50_ns: p50[2],
            headline_p95_ns: p95[2],
            rounds,
        });
    }
    let selection = evaluate_mask_candidates(&candidates)?;
    let resources_after = resource_usage()?;
    if executable_identity()? != executable {
        return Err(MaskBuildError::new(
            "BENCHMARK_EXECUTABLE",
            "benchmark executable changed during measurement",
        ));
    }
    Ok(MaskBenchmarkReport {
        schema: REPORT_SCHEMA.into(),
        profile: MASK_PROFILE.into(),
        contract_id: input.contract_id.clone(),
        builder_source_sha256: BUILDER_SOURCE_SHA256.into(),
        performance_manifest: input.performance_identity.clone(),
        method: BenchmarkMethod::ticket_012(),
        host,
        resources: BenchmarkResources {
            maximum_rss_bytes: resources_after.maximum_rss_bytes,
            minor_faults: resources_after
                .minor_faults
                .saturating_sub(resources_before.minor_faults),
            major_faults: resources_after
                .major_faults
                .saturating_sub(resources_before.major_faults),
        },
        candidates,
        selection,
    })
}

struct PageMetrics {
    sha256: String,
    metadata_pages: u64,
    median_payload_pages: u64,
    p95_payload_pages: u64,
}

struct QueryPreflight {
    gene_capacity: usize,
    boundary_capacity: usize,
    pages: BTreeMap<MaskCandidateCodec, PageMetrics>,
    queries: Vec<TypedPerformanceQuery>,
}

#[derive(Clone, Copy)]
struct TypedPerformanceQuery {
    ordinal: u16,
    contig: Grch38Contig,
    position: GenomicPosition,
}

fn preflight_queries(input: &BenchmarkRunInput) -> Result<QueryPreflight, MaskBuildError> {
    let queries = input
        .performance_manifest
        .queries
        .iter()
        .map(|query| {
            Ok(TypedPerformanceQuery {
                ordinal: query.ordinal,
                contig: Grch38Contig::from_str(&query.contig).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "contig is invalid")
                })?,
                position: GenomicPosition::new(query.position).map_err(|_| {
                    MaskBuildError::new("PERFORMANCE_MANIFEST", "position is invalid")
                })?,
            })
        })
        .collect::<Result<Vec<_>, MaskBuildError>>()?;
    let mut maximum_genes = 0_usize;
    let mut maximum_boundaries = 0_usize;
    let mut results = BTreeMap::new();
    for codec in MaskCandidateCodec::ALL {
        let candidate = candidate_input(input, codec)?;
        let mut authenticated = open_candidate_for_benchmark(candidate)?;
        let reader = MaskCandidateReader::open_held(authenticated.reader_file()?)?;
        reader.inspect_payload_with_cancellation(&|| CANCELLED.load(Ordering::SeqCst))?;
        let mut output = MaskQueryBuffer::with_capacity(64, 4_096);
        let mut payload_counts = Vec::with_capacity(1_000);
        let mut metadata_pages = None;
        let mut trace_hasher = Sha256::new();
        for (query, manifest_query) in queries.iter().zip(&input.performance_manifest.queries) {
            cancelled()?;
            let trace = reader.query_with_page_trace(query.contig, query.position, &mut output)?;
            if query_digest(&output)? != manifest_query.expected_sha256 {
                return Err(MaskBuildError::new(
                    "BENCHMARK_ORACLE",
                    "candidate query differs from the performance oracle",
                ));
            }
            let current_metadata = trace.metadata_pages.len() as u64;
            if metadata_pages
                .replace(current_metadata)
                .is_some_and(|prior| prior != current_metadata)
            {
                return Err(MaskBuildError::new(
                    "PAGE_TRACE",
                    "metadata page trace is not deterministic",
                ));
            }
            payload_counts.push(trace.payload_pages.len() as u64);
            let encoded = serde_jcs::to_vec(&TraceRecord {
                ordinal: query.ordinal,
                metadata_pages: &trace.metadata_pages,
                payload_pages: &trace.payload_pages,
            })
            .map_err(|_| MaskBuildError::new("PAGE_TRACE", "page trace encoding failed"))?;
            trace_hasher.update((encoded.len() as u64).to_le_bytes());
            trace_hasher.update(encoded);
            maximum_genes = maximum_genes.max(output.plus().len() + output.minus().len());
            maximum_genes = maximum_genes.max(output.scratch_match_count());
            maximum_boundaries = maximum_boundaries.max(
                output
                    .plus()
                    .iter()
                    .chain(output.minus())
                    .map(|gene| output.boundaries(gene).len())
                    .sum(),
            );
        }
        payload_counts.sort_unstable();
        cancelled()?;
        results.insert(
            codec,
            PageMetrics {
                sha256: format!("{:x}", trace_hasher.finalize()),
                metadata_pages: metadata_pages.unwrap_or(0),
                median_payload_pages: payload_counts[499],
                p95_payload_pages: payload_counts[949],
            },
        );
        authenticated.reauthenticate()?;
    }
    Ok(QueryPreflight {
        gene_capacity: maximum_genes.max(1),
        boundary_capacity: maximum_boundaries.max(1),
        pages: results,
        queries,
    })
}

fn candidate_input(
    input: &BenchmarkRunInput,
    codec: MaskCandidateCodec,
) -> Result<&pangopup_build::mask::CandidateRunInput, MaskBuildError> {
    input
        .candidates
        .iter()
        .find(|candidate| candidate.codec == codec)
        .ok_or_else(|| MaskBuildError::new("BENCHMARK", "candidate input is missing"))
}

fn point_query(
    reader: &MaskCandidateReader,
    query: TypedPerformanceQuery,
    output: &mut MaskQueryBuffer,
) -> Result<(), MaskBuildError> {
    reader.query(query.contig, query.position, output)?;
    Ok(())
}

fn query_digest(output: &MaskQueryBuffer) -> Result<String, MaskBuildError> {
    let convert = |values: &[pangopup_index::mask_candidates::MaskQueryGene]| {
        values
            .iter()
            .map(|gene| QueryGene {
                id: gene.identity().to_string(),
                boundaries: output
                    .boundaries(gene)
                    .iter()
                    .map(|boundary| boundary.get())
                    .collect(),
            })
            .collect()
    };
    let mut bytes = serde_jcs::to_vec(&QueryValue {
        plus: convert(output.plus()),
        minus: convert(output.minus()),
    })
    .map_err(|_| MaskBuildError::new("BENCHMARK_ORACLE", "query encoding failed"))?;
    bytes.push(b'\n');
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn query_checksum(output: &MaskQueryBuffer) -> u64 {
    output
        .plus()
        .iter()
        .chain(output.minus())
        .fold(0_u64, |checksum, gene| {
            checksum
                .wrapping_mul(1_099_511_628_211)
                .wrapping_add(gene.identity().stable().numeric())
                .wrapping_add(gene.identity().version().get() as u64)
                .wrapping_add(output.boundaries(gene).len() as u64)
        })
}

fn begin_open_tracking() {
    let current = CURRENT_BYTES.load(Ordering::SeqCst);
    OPEN_BASELINE.store(current, Ordering::SeqCst);
    OPEN_PEAK.store(current, Ordering::SeqCst);
    TRACK_OPEN.store(true, Ordering::SeqCst);
}

fn end_open_tracking() -> u64 {
    TRACK_OPEN.store(false, Ordering::SeqCst);
    OPEN_PEAK
        .load(Ordering::SeqCst)
        .saturating_sub(OPEN_BASELINE.load(Ordering::SeqCst))
}

fn reset_allocation_tracking() {
    ALLOCATION_CALLS.store(0, Ordering::SeqCst);
    ALLOCATION_BYTES.store(0, Ordering::SeqCst);
}

struct AllocationTracking;

impl AllocationTracking {
    fn begin() -> Self {
        TRACK_ALLOCATIONS.store(true, Ordering::SeqCst);
        Self
    }
}

impl Drop for AllocationTracking {
    fn drop(&mut self) {
        TRACK_ALLOCATIONS.store(false, Ordering::SeqCst);
    }
}

fn elapsed_ns(started: Instant) -> Result<u64, MaskBuildError> {
    u64::try_from(started.elapsed().as_nanos())
        .map_err(|_| MaskBuildError::new("BENCHMARK", "duration overflow"))
}

fn zstd_size(file: &File, size: u64) -> Result<u64, MaskBuildError> {
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
    let mut encoder = zstd::stream::Encoder::new(CountingSink(0), 9)
        .map_err(|_| MaskBuildError::new("ZSTD", "zstd encoder failed"))?;
    encoder
        .include_checksum(true)
        .and_then(|_| encoder.include_contentsize(true))
        .and_then(|_| encoder.include_dictid(false))
        .and_then(|_| encoder.long_distance_matching(false))
        .and_then(|_| encoder.multithread(0))
        .and_then(|_| encoder.set_pledged_src_size(Some(size)))
        .map_err(|_| MaskBuildError::new("ZSTD", "zstd parameters failed"))?;
    let mut source = file
        .try_clone()
        .map_err(|_| MaskBuildError::new("ZSTD", "candidate clone failed"))?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| MaskBuildError::new("ZSTD", "candidate seek failed"))?;
    let mut copied = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        cancelled()?;
        let read = std::io::Read::read(&mut source, &mut buffer)
            .map_err(|_| MaskBuildError::new("ZSTD", "candidate read failed"))?;
        if read == 0 {
            break;
        }
        encoder
            .write_all(&buffer[..read])
            .map_err(|_| MaskBuildError::new("ZSTD", "zstd encode failed"))?;
        copied = copied
            .checked_add(read as u64)
            .ok_or_else(|| MaskBuildError::new("ZSTD", "candidate size overflow"))?;
    }
    if copied != size {
        return Err(MaskBuildError::new(
            "ZSTD",
            "candidate size changed during compression",
        ));
    }
    Ok(encoder
        .finish()
        .map_err(|_| MaskBuildError::new("ZSTD", "zstd finish failed"))?
        .0)
}

fn resource_usage() -> Result<ResourceUsage, MaskBuildError> {
    let mut value = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied object when it succeeds.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, value.as_mut_ptr()) } != 0 {
        return Err(MaskBuildError::new("RESOURCE", "getrusage failed"));
    }
    // SAFETY: the successful call above initialized the object.
    let value = unsafe { value.assume_init() };
    Ok(ResourceUsage {
        maximum_rss_bytes: (value.ru_maxrss as u64).saturating_mul(1_024),
        minor_faults: value.ru_minflt as u64,
        major_faults: value.ru_majflt as u64,
    })
}

fn pin_and_describe_host(executable: Identity) -> Result<BenchmarkHost, MaskBuildError> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err(MaskBuildError::new(
            "AFFINITY",
            "the qualification benchmark requires Linux",
        ));
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: libc CPU-set functions receive initialized cpu_set_t values.
        unsafe {
            let mut inherited: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut inherited);
            if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut inherited)
                != 0
            {
                return Err(MaskBuildError::new("AFFINITY", "affinity query failed"));
            }
            let allowed = (0..libc::CPU_SETSIZE as usize)
                .filter(|cpu| libc::CPU_ISSET(*cpu, &inherited))
                .collect::<Vec<_>>();
            let selected = *allowed
                .first()
                .ok_or_else(|| MaskBuildError::new("AFFINITY", "allowed CPU set is empty"))?;
            let mut pinned: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut pinned);
            libc::CPU_SET(selected, &mut pinned);
            if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &pinned) != 0 {
                return Err(MaskBuildError::new("AFFINITY", "affinity pin failed"));
            }
            let mut observed: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut observed);
            if libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut observed)
                != 0
                || (0..libc::CPU_SETSIZE as usize)
                    .filter(|cpu| libc::CPU_ISSET(*cpu, &observed))
                    .collect::<Vec<_>>()
                    != vec![selected]
            {
                return Err(MaskBuildError::new(
                    "AFFINITY",
                    "single-CPU affinity did not hold",
                ));
            }
            let page = libc::sysconf(libc::_SC_PAGESIZE);
            if page != 4_096 {
                return Err(MaskBuildError::new(
                    "HOST",
                    "logical page size is not 4,096 bytes",
                ));
            }
            Ok(BenchmarkHost {
                selected_cpu: selected as u32,
                allowed_cpu_count_before_pin: allowed.len() as u32,
                cpu_model: cpu_model()?,
                kernel: kernel()?,
                governor: read_host_value(&format!(
                    "/sys/devices/system/cpu/cpu{selected}/cpufreq/scaling_governor"
                )),
                power_state: read_host_value(&format!(
                    "/sys/devices/system/cpu/cpu{selected}/cpufreq/energy_performance_preference"
                )),
                rustc: env!("PANGOPUP_RUSTC_VERSION").into(),
                target: env!("PANGOPUP_TARGET").into(),
                build_profile: env!("PANGOPUP_BUILD_PROFILE").into(),
                executable,
                logical_page_bytes: page as u64,
            })
        }
    }
}

fn executable_identity() -> Result<Identity, MaskBuildError> {
    let mut file = File::open("/proc/self/exe").map_err(|_| {
        MaskBuildError::new(
            "BENCHMARK_EXECUTABLE",
            "benchmark executable is unavailable",
        )
    })?;
    let metadata = file.metadata().map_err(|_| {
        MaskBuildError::new(
            "BENCHMARK_EXECUTABLE",
            "benchmark executable metadata is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 256 * 1024 * 1024 {
        return Err(MaskBuildError::new(
            "BENCHMARK_EXECUTABLE",
            "benchmark executable is not a bounded regular file",
        ));
    }
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer).map_err(|_| {
            MaskBuildError::new("BENCHMARK_EXECUTABLE", "benchmark executable read failed")
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        bytes = bytes
            .checked_add(read as u64)
            .ok_or_else(|| MaskBuildError::new("BENCHMARK_EXECUTABLE", "size overflow"))?;
    }
    if bytes != metadata.len() {
        return Err(MaskBuildError::new(
            "BENCHMARK_EXECUTABLE",
            "benchmark executable changed while reading",
        ));
    }
    Ok(Identity {
        bytes,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

fn read_host_value(path: &str) -> String {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| sanitize_host_value(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn sanitize_host_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || " ._-()/".contains(*character))
        .take(256)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn cpu_model() -> Result<String, MaskBuildError> {
    let value = std::fs::read_to_string("/proc/cpuinfo")
        .map_err(|_| MaskBuildError::new("HOST", "CPU model is unavailable"))?;
    for line in value.lines() {
        if let Some((key, model)) = line.split_once(':')
            && matches!(key.trim(), "model name" | "Hardware")
        {
            let model = sanitize_host_value(model);
            if !model.is_empty() {
                return Ok(model);
            }
        }
    }
    Err(MaskBuildError::new("HOST", "CPU model is unavailable"))
}

fn kernel() -> Result<String, MaskBuildError> {
    let mut value = std::mem::MaybeUninit::<libc::utsname>::zeroed();
    // SAFETY: uname initializes the supplied object when successful.
    if unsafe { libc::uname(value.as_mut_ptr()) } != 0 {
        return Err(MaskBuildError::new(
            "HOST",
            "kernel identity is unavailable",
        ));
    }
    // SAFETY: the successful call above initialized the object and release is NUL-terminated.
    let value = unsafe { value.assume_init() };
    let release = unsafe { std::ffi::CStr::from_ptr(value.release.as_ptr()) }.to_string_lossy();
    let result = sanitize_host_value(&release);
    if result.is_empty() {
        Err(MaskBuildError::new(
            "HOST",
            "kernel identity is unavailable",
        ))
    } else {
        Ok(result)
    }
}

extern "C" fn cancellation_handler(_: libc::c_int) {
    CANCELLED.store(true, Ordering::SeqCst);
    request_cancellation();
}

fn install_cancellation_handlers() {
    // SAFETY: the handler only sets a lock-free atomic flag.
    unsafe {
        libc::signal(
            libc::SIGINT,
            cancellation_handler as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            cancellation_handler as *const () as libc::sighandler_t,
        );
    }
}

fn cancelled() -> Result<(), MaskBuildError> {
    if CANCELLED.load(Ordering::SeqCst) {
        Err(MaskBuildError::new(
            "CANCELLED",
            "qualification was cancelled; automatic retry is forbidden",
        ))
    } else {
        Ok(())
    }
}
