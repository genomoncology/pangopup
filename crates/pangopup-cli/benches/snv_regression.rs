use pangopup_cli::{OutputFormat, RenderRequest, render_requests};
use pangopup_core::{DnaBase, EnsemblGeneId, GenomicPosition, Grch38Snv, ScoreProvider};
use pangopup_index::BundleOpen;
use std::{
    alloc::{GlobalAlloc, Layout, System},
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

// SAFETY: every operation delegates to System; relaxed counters are diagnostic.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: GlobalAlloc supplies a valid layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pair came from the delegated allocator.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone)]
struct Query {
    variant: String,
    snv: Grch38Snv,
    gene: Option<EnsemblGeneId>,
}

#[derive(Clone, Copy)]
struct Usage {
    minor: i64,
    major: i64,
    rss_kib: i64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/snv-regression");
    let bundle = fixture.join("bundle");
    let provider = BundleOpen::open(&bundle)?;
    let queries = load_queries(&fixture.join("requests.tsv"), &provider)?;
    println!(
        "mode\trequests\tresults\tp50_us\tp95_us\tp99_us\talloc_calls\talloc_bytes\tminor_faults\tmajor_faults\trss_delta_kib\toutput_bytes"
    );
    sample("fresh-open", 0, 25, || {
        black_box(BundleOpen::open(&bundle).expect("fresh open"));
        (0, 0)
    });
    let cli = cli_path()?;
    sample("fresh-process", 1, 10, || {
        let output = Command::new(&cli)
            .arg("lookup")
            .arg("--bundle")
            .arg(&bundle)
            .arg("--variant")
            .arg(&queries[0].variant)
            .arg("--gene")
            .arg(
                queries[0]
                    .gene
                    .expect("first query is filtered")
                    .to_string(),
            )
            .output()
            .expect("fresh CLI process");
        assert!(output.status.success());
        (1, output.stdout.len())
    });
    for size in [1_usize, 10, 100, 1_000] {
        sample("warm-provider-render", size, 100, || {
            let rendered = queries[..size]
                .iter()
                .map(|query| {
                    let result = provider.lookup(query.snv, query.gene).expect("lookup");
                    RenderRequest::new(query.snv, result)
                })
                .collect::<Vec<_>>();
            let results = rendered
                .iter()
                .map(|request| {
                    request.result().records().len()
                        + request.result().source_reference_ambiguities().len()
                })
                .sum();
            let output = render_requests(OutputFormat::Jsonl, &rendered).expect("render");
            black_box(&output);
            (results, output.len())
        });
    }
    eprintln!(
        "RSS is ru_maxrss high-water delta; page-cache residency is uncontrolled, so no row is labeled cold."
    );
    Ok(())
}

fn load_queries(path: &Path, provider: &BundleOpen) -> Result<Vec<Query>, Box<dyn Error>> {
    fs::read_to_string(path)?
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let variant = fields[3].to_owned();
            let parts: Vec<_> = variant.split(':').collect();
            let (contig, length) = provider.resolve_contig(parts[1]).ok_or("contig")?;
            let position = parts[2].parse::<u32>()?;
            if position > length {
                return Err("position".into());
            }
            let snv = Grch38Snv::new(
                contig,
                GenomicPosition::new(position)?,
                DnaBase::parse(parts[3])?,
                DnaBase::parse(parts[4])?,
            )?;
            let gene = if fields[4] == "." {
                None
            } else {
                Some(EnsemblGeneId::from_str(fields[4])?)
            };
            Ok(Query { variant, snv, gene })
        })
        .collect()
}

fn cli_path() -> Result<PathBuf, Box<dyn Error>> {
    let executable = env::current_exe()?;
    let target = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("benchmark target directory")?;
    let cli = target.join("debug/pangopup");
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--package",
            "pangopup-cli",
            "--bin",
            "pangopup",
        ])
        .status()?;
    if !status.success() || !cli.is_file() {
        return Err("could not build the fresh-process benchmark executable".into());
    }
    Ok(cli)
}

fn usage() -> Usage {
    let mut value = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage initializes the value when it returns zero.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, value.as_mut_ptr()) } != 0 {
        return Usage {
            minor: 0,
            major: 0,
            rss_kib: 0,
        };
    }
    // SAFETY: success above proves initialization.
    let value = unsafe { value.assume_init() };
    Usage {
        minor: value.ru_minflt,
        major: value.ru_majflt,
        rss_kib: value.ru_maxrss,
    }
}

fn sample(
    mode: &str,
    requests: usize,
    samples: usize,
    mut operation: impl FnMut() -> (usize, usize),
) {
    for _ in 0..5 {
        black_box(operation());
    }
    let mut times = Vec::with_capacity(samples);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    let before = usage();
    let mut results = 0;
    let mut output_bytes = 0;
    for _ in 0..samples {
        let start = Instant::now();
        (results, output_bytes) = operation();
        times.push(start.elapsed().as_nanos());
    }
    let after = usage();
    times.sort_unstable();
    // Nearest-rank percentile: sort ascending and select ceil(p * n) - 1.
    let percentile = |numerator: usize| {
        let rank = (numerator * times.len()).div_ceil(100).saturating_sub(1);
        times[rank] / 1_000
    };
    println!(
        "{mode}\t{requests}\t{results}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{output_bytes}",
        percentile(50),
        percentile(95),
        percentile(99),
        ALLOCATIONS.load(Ordering::Relaxed) / samples as u64,
        ALLOCATED_BYTES.load(Ordering::Relaxed) / samples as u64,
        (after.minor - before.minor) / samples as i64,
        (after.major - before.major) / samples as i64,
        after.rss_kib - before.rss_kib,
    );
}
