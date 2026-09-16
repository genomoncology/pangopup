use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, PangolinScore, RelativePosition,
    ScoreMagnitude,
};
use pangopup_index::{
    InputAlternative, InputLocus, OrdinaryInputLocus, sparse_writer::SparseIndexWriter,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(target_os = "linux")]
use std::sync::{Arc, atomic::AtomicBool};

struct TrackingAllocator;

static CURRENT: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

fn add_allocation(bytes: usize) {
    let current = CURRENT.fetch_add(bytes as u64, Ordering::SeqCst) + bytes as u64;
    PEAK.fetch_max(current, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegates the unchanged layout to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: `pointer` and `layout` are the pair supplied by the caller.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Delegates the unchanged allocation pair and requested size.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                add_allocation(new_size - layout.size());
            } else {
                CURRENT.fetch_sub((layout.size() - new_size) as u64, Ordering::SeqCst);
            }
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("pangopup-sparse-heap-bound-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create heap regression directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove heap regression directory");
    }
}

#[cfg(target_os = "linux")]
fn resident_bytes() -> u64 {
    let statm = fs::read_to_string("/proc/self/statm").expect("read process RSS");
    let pages: u64 = statm
        .split_ascii_whitespace()
        .nth(1)
        .expect("resident pages")
        .parse()
        .expect("numeric resident pages");
    // SAFETY: `_SC_PAGESIZE` is a side-effect-free process query.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    assert!(page_size > 0);
    pages * page_size as u64
}

#[cfg(not(target_os = "linux"))]
fn resident_bytes() -> u64 {
    0
}

#[cfg(target_os = "linux")]
struct RssSampler {
    running: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl RssSampler {
    fn start(baseline: u64) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let peak = Arc::new(AtomicU64::new(0));
        let thread_running = Arc::clone(&running);
        let thread_peak = Arc::clone(&peak);
        let handle = std::thread::spawn(move || {
            while thread_running.load(Ordering::SeqCst) {
                thread_peak.fetch_max(resident_bytes().saturating_sub(baseline), Ordering::SeqCst);
                std::thread::yield_now();
            }
        });
        Self {
            running,
            peak,
            handle: Some(handle),
        }
    }

    fn finish(mut self) -> u64 {
        self.running.store(false, Ordering::SeqCst);
        self.handle
            .take()
            .expect("RSS sampler handle")
            .join()
            .expect("RSS sampler thread");
        self.peak.load(Ordering::SeqCst)
    }
}

#[cfg(not(target_os = "linux"))]
struct RssSampler;

#[cfg(not(target_os = "linux"))]
impl RssSampler {
    fn start(_baseline: u64) -> Self {
        Self
    }

    fn finish(self) -> u64 {
        0
    }
}

#[test]
fn sparse_writer_heap_and_rss_stay_bounded_through_finish() {
    const GENES: u64 = 1_500;
    const LOCI_PER_GENE: u32 = 1_000;

    let temp = Temp::new();
    let scratch = temp.0.join("candidate.scratch");
    let output = temp.0.join("candidate.pgi");
    let baseline_allocated = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline_allocated, Ordering::SeqCst);
    let baseline_rss = resident_bytes();
    let rss_sampler = RssSampler::start(baseline_rss);
    let mut max_retained = 0_u64;
    let mut max_rss = 0_u64;
    let mut writer = SparseIndexWriter::create(&scratch).expect("create sparse writer");
    sample(
        baseline_allocated,
        baseline_rss,
        &mut max_retained,
        &mut max_rss,
    );
    let gain = ScoreMagnitude::new(100).expect("gain");
    let loss = ScoreMagnitude::new(99).expect("loss");
    let gain_position = RelativePosition::new(50).expect("gain position");
    let loss_position = RelativePosition::new(-49).expect("loss position");
    let score = PangolinScore::new(gain, gain_position, loss, loss_position);
    for numeric in 1..=GENES {
        let gene = EnsemblGeneId::from_numeric(numeric).expect("gene");
        let loci: Vec<_> = (1..=LOCI_PER_GENE)
            .map(|position| {
                let alternatives = [DnaBase::C, DnaBase::G, DnaBase::T]
                    .map(|alternate| InputAlternative { alternate, score });
                InputLocus::Ordinary(OrdinaryInputLocus {
                    gene,
                    contig: Grch38Contig::from_code(1).expect("contig"),
                    position: GenomicPosition::new(position).expect("position"),
                    reference: DnaBase::A,
                    alternatives,
                })
            })
            .collect();
        writer.push_gene(&loci).expect("spool complete gene");
        drop(loci);
        sample(
            baseline_allocated,
            baseline_rss,
            &mut max_retained,
            &mut max_rss,
        );
    }

    let scratch_bytes = writer.scratch_bytes().expect("scratch byte count");
    let summary = writer.finish(&output).expect("finish sparse writer");
    sample(
        baseline_allocated,
        baseline_rss,
        &mut max_retained,
        &mut max_rss,
    );
    max_rss = max_rss.max(rss_sampler.finish());
    let retained_after_finish = CURRENT
        .load(Ordering::SeqCst)
        .saturating_sub(baseline_allocated);
    let peak_delta = PEAK
        .load(Ordering::SeqCst)
        .saturating_sub(baseline_allocated);
    assert_eq!(summary.loci, GENES * u64::from(LOCI_PER_GENE));
    assert_eq!(
        fs::metadata(&output).expect("output metadata").len(),
        summary.bytes
    );
    assert_eq!(scratch_bytes, summary.bytes - 256 - GENES * (32 + 48 + 40));
    assert!(!scratch.exists());
    assert!(
        max_retained < summary.bytes / 8,
        "retained heap {max_retained} is too close to {} output bytes",
        summary.bytes
    );
    assert!(
        retained_after_finish < summary.bytes / 16,
        "finish retained {retained_after_finish} bytes for {} output bytes",
        summary.bytes
    );
    assert!(
        peak_delta < summary.bytes / 8,
        "peak heap {peak_delta} suggests final assembly collected {} bytes",
        summary.bytes
    );
    #[cfg(target_os = "linux")]
    assert!(
        max_rss < summary.bytes / 2,
        "RSS growth {max_rss} suggests retained payload for {} output bytes",
        summary.bytes
    );
    eprintln!(
        "sparse-heap-bound output_bytes={} scratch_bytes={scratch_bytes} max_retained={max_retained} retained_after_finish={retained_after_finish} peak_delta={peak_delta} max_rss={max_rss}",
        summary.bytes
    );
}

fn sample(baseline_heap: u64, baseline_rss: u64, max_heap: &mut u64, max_rss: &mut u64) {
    *max_heap = (*max_heap).max(CURRENT.load(Ordering::SeqCst).saturating_sub(baseline_heap));
    *max_rss = (*max_rss).max(resident_bytes().saturating_sub(baseline_rss));
}
