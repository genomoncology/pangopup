use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, ScoreProvider};
use pangopup_index::BundleOpen;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

struct CountingAllocator;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: this delegates every operation to the process System allocator and
// only adds relaxed diagnostic counters around successful allocation requests.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: the caller supplied the layout under GlobalAlloc's contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer/layout pair comes from the delegated allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone)]
struct Query {
    order: u64,
    variant: String,
    snv: Grch38Snv,
    gene: Option<EnsemblGeneId>,
    expected_records: usize,
}

#[derive(Clone, Copy)]
struct Usage {
    minor: i64,
    major: i64,
    rss_kib: i64,
}

fn usage() -> Usage {
    let mut value = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the supplied rusage on success.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, value.as_mut_ptr()) };
    if status != 0 {
        return Usage {
            minor: 0,
            major: 0,
            rss_kib: 0,
        };
    }
    // SAFETY: status zero proves initialization.
    let value = unsafe { value.assume_init() };
    Usage {
        minor: value.ru_minflt,
        major: value.ru_majflt,
        rss_kib: value.ru_maxrss,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let bundle_path = PathBuf::from(env::var("PANGOPUP_BUNDLE")?);
    let manifest_path = PathBuf::from(env::var("PANGOPUP_QUERY_MANIFEST")?);
    let cli = PathBuf::from(env::var("PANGOPUP_CLI")?);
    let groups = load_queries(&manifest_path, &bundle_path)?;
    assert_sample_harness_is_unmeasured();
    println!(
        "mode\tworkload\tsamples\trequests\trecords\tp50_us\tp95_us\tp99_us\tbatches_s\trecords_s\talloc_calls\talloc_bytes\tminor_faults\tmajor_faults\trss_delta_kib\tlogical_bytes\tlogical_pages\toutput_bytes"
    );
    benchmark_open(&bundle_path)?;
    let provider = BundleOpen::open(&bundle_path)?;
    for (name, queries) in groups {
        let materialized = materialize(&provider, &queries);
        let expected_cli = render_requests(OutputFormat::Jsonl, &materialized)?;
        benchmark_cli(&cli, &bundle_path, &name, &queries, &expected_cli)?;
        benchmark_lookup(&provider, &name, &queries)?;
        benchmark_serialization(&name, &materialized, OutputFormat::Jsonl)?;
        benchmark_serialization(&name, &materialized, OutputFormat::Table)?;
    }
    Ok(())
}

fn load_queries(
    path: &Path,
    bundle_path: &Path,
) -> Result<BTreeMap<String, Vec<Query>>, Box<dyn Error>> {
    let provider = BundleOpen::open(bundle_path)?;
    let text = fs::read_to_string(path)?;
    let mut groups = BTreeMap::new();
    for line in text.lines().skip(1) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7 {
            return Err("query manifest must have seven TSV columns".into());
        }
        let mut parts = fields[3].split(':');
        if parts.next() != Some("GRCh38") {
            return Err("query assembly".into());
        }
        let contig_text = parts.next().ok_or("query contig")?;
        let position = parts.next().ok_or("query position")?.parse::<u32>()?;
        let reference = DnaBase::parse(parts.next().ok_or("query REF")?)?;
        let alternate = DnaBase::parse(parts.next().ok_or("query ALT")?)?;
        if parts.next().is_some() {
            return Err("query tuple width".into());
        }
        let (contig, length) = provider
            .resolve_contig(contig_text)
            .ok_or("query contig alias")?;
        if position > length {
            return Err("query position exceeds contig".into());
        }
        let snv = Grch38Snv::new(
            contig,
            GenomicPosition::new(position)?,
            reference,
            alternate,
        )?;
        let gene = if fields[4] == "." {
            None
        } else {
            Some(EnsemblGeneId::from_str(fields[4])?)
        };
        groups
            .entry(fields[1].to_owned())
            .or_insert_with(Vec::new)
            .push(Query {
                order: fields[2].parse()?,
                variant: fields[3].to_owned(),
                snv,
                gene,
                expected_records: fields[6].parse()?,
            });
    }
    for values in groups.values_mut() {
        values.sort_by_key(|query| query.order);
    }
    Ok(groups)
}

fn reset_allocations() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
}
fn allocations() -> (u64, u64) {
    (
        ALLOCATIONS.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
    )
}

fn sample(mut operation: impl FnMut() -> usize) -> (Vec<u128>, usize, u64, u64, Usage) {
    // Allocate and touch retained-sample storage before either the allocation
    // counters or process-resource baseline. Only the measured operation may
    // contribute to the reported allocation/fault/RSS deltas.
    let mut values = vec![0; 100];
    for _ in 0..20 {
        black_box(operation());
    }
    reset_allocations();
    let before = usage();
    assert_eq!(
        allocations(),
        (0, 0),
        "sample harness allocated inside the measurement window"
    );
    let mut bytes = 0;
    for value in &mut values {
        let start = Instant::now();
        bytes = operation();
        *value = start.elapsed().as_nanos();
    }
    let after = usage();
    let (calls, allocated) = allocations();
    (
        values,
        bytes,
        calls,
        allocated,
        Usage {
            minor: after.minor - before.minor,
            major: after.major - before.major,
            rss_kib: after.rss_kib - before.rss_kib,
        },
    )
}

fn assert_sample_harness_is_unmeasured() {
    let (_, _, calls, bytes, _) = sample(|| 0);
    assert_eq!(
        (calls, bytes),
        (0, 0),
        "empty benchmark operation must report zero allocations"
    );
}

fn benchmark_open(bundle: &Path) -> Result<(), Box<dyn Error>> {
    let (times, _, calls, bytes, delta) = sample(|| {
        black_box(BundleOpen::open(bundle).expect("open"));
        0
    });
    report(
        "open-only",
        "fresh",
        0,
        0,
        &times,
        calls,
        bytes,
        delta,
        "N/A",
        "N/A",
        "N/A",
    );
    Ok(())
}

fn benchmark_lookup(
    provider: &BundleOpen,
    name: &str,
    queries: &[Query],
) -> Result<(), Box<dyn Error>> {
    let expected: usize = queries.iter().map(|query| query.expected_records).sum();
    let metrics = provider.lookup_batch_measured(
        &queries
            .iter()
            .map(|query| (query.snv, query.gene))
            .collect::<Vec<_>>(),
    )?;
    let (times, _, calls, bytes, delta) = sample(|| {
        let records: usize = queries
            .iter()
            .map(|query| {
                provider
                    .lookup(query.snv, query.gene)
                    .expect("lookup")
                    .records()
                    .len()
            })
            .sum();
        assert_eq!(records, expected);
        black_box(records)
    });
    report(
        "lookup-only",
        name,
        queries.len(),
        expected,
        &times,
        calls,
        bytes,
        delta,
        &metrics.logical_bytes_decoded.to_string(),
        &metrics.unique_mapped_pages_addressed.to_string(),
        "N/A",
    );
    Ok(())
}

fn materialize(provider: &BundleOpen, queries: &[Query]) -> Vec<RenderRequest> {
    queries
        .iter()
        .map(|query| {
            RenderRequest::new(
                query.snv,
                provider.lookup(query.snv, query.gene).expect("lookup"),
            )
        })
        .collect()
}

fn benchmark_serialization(
    name: &str,
    materialized: &[RenderRequest],
    format: OutputFormat,
) -> Result<(), Box<dyn Error>> {
    let records: usize = materialized
        .iter()
        .map(|request| request.result().records().len())
        .sum();
    let (times, output, calls, bytes, delta) = sample(|| {
        let rendered = render_requests(format, materialized).expect("render");
        black_box(rendered.len())
    });
    let format_name = match format {
        OutputFormat::Jsonl => "jsonl",
        OutputFormat::Table => "table",
    };
    report(
        &format!("serialization-{format_name}"),
        name,
        materialized.len(),
        records,
        &times,
        calls,
        bytes,
        delta,
        "N/A",
        "N/A",
        &output.to_string(),
    );
    Ok(())
}

fn benchmark_cli(
    cli: &Path,
    bundle: &Path,
    name: &str,
    queries: &[Query],
    expected_output: &[u8],
) -> Result<(), Box<dyn Error>> {
    let records: usize = queries.iter().map(|query| query.expected_records).sum();
    let gene = queries.first().and_then(|query| query.gene);
    if queries.iter().any(|query| query.gene != gene) {
        return Err("CLI batch requires one global gene".into());
    }
    let time_path = env::temp_dir().join(format!("pangopup-benchmark-time-{}", std::process::id()));
    let command = || {
        let mut child = Command::new("/usr/bin/time");
        child
            .arg("-f")
            .arg("%R\t%F\t%M")
            .arg("-o")
            .arg(&time_path)
            .arg(cli);
        child.arg("lookup").arg("--bundle").arg(bundle);
        for query in queries {
            child.arg("--variant").arg(&query.variant);
        }
        if let Some(gene) = gene {
            child.arg("--gene").arg(gene.to_string());
        }
        let output = child.output().expect("CLI child");
        let usage = fs::read_to_string(&time_path).expect("GNU time output");
        let fields: Vec<_> = usage.trim().split('\t').collect();
        assert_eq!(fields.len(), 3);
        let child_usage = Usage {
            minor: fields[0].parse().expect("child minor faults"),
            major: fields[1].parse().expect("child major faults"),
            rss_kib: fields[2].parse().expect("child peak RSS"),
        };
        (output, child_usage)
    };
    for _ in 0..20 {
        let (output, _) = command();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected_output);
    }
    let mut times = Vec::with_capacity(100);
    let mut output_bytes = 0;
    let mut child_minor = 0;
    let mut child_major = 0;
    let mut child_peak_rss = 0;
    for _ in 0..100 {
        let start = Instant::now();
        let (output, child_usage) = command();
        times.push(start.elapsed().as_nanos());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected_output);
        output_bytes = output.stdout.len();
        child_minor += child_usage.minor;
        child_major += child_usage.major;
        child_peak_rss = child_peak_rss.max(child_usage.rss_kib);
    }
    let mut sorted = times;
    sorted.sort_unstable();
    let p50 = sorted[49] as f64 / 1000.0;
    let p95 = sorted[94] as f64 / 1000.0;
    let p99 = sorted[98] as f64 / 1000.0;
    let batches = 1_000_000.0 / p50;
    println!(
        "fresh-cli\t{name}\t100\t{}\t{records}\t{p50:.3}\t{p95:.3}\t{p99:.3}\t{batches:.3}\t{:.3}\tN/A\tN/A\t{child_minor}\t{child_major}\t{child_peak_rss}\tN/A\tN/A\t{output_bytes}",
        queries.len(),
        batches * records as f64
    );
    fs::remove_file(time_path)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn report(
    mode: &str,
    workload: &str,
    requests: usize,
    records: usize,
    times: &[u128],
    calls: u64,
    bytes: u64,
    usage: Usage,
    logical: &str,
    pages: &str,
    output: &str,
) {
    let mut sorted = times.to_vec();
    sorted.sort_unstable();
    let p50 = sorted[49] as f64 / 1000.0;
    let p95 = sorted[94] as f64 / 1000.0;
    let p99 = sorted[98] as f64 / 1000.0;
    let batches = 1_000_000.0 / p50;
    let records_s = batches * records as f64;
    let calls_per_batch = calls as f64 / 100.0;
    let bytes_per_batch = bytes as f64 / 100.0;
    println!(
        "{mode}\t{workload}\t100\t{requests}\t{records}\t{p50:.3}\t{p95:.3}\t{p99:.3}\t{batches:.3}\t{records_s:.3}\t{calls_per_batch:.2}\t{bytes_per_batch:.2}\t{}\t{}\t{}\t{logical}\t{pages}\t{output}",
        usage.minor, usage.major, usage.rss_kib
    );
}
