use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, PangolinScore, RelativePosition,
    ScoreMagnitude,
};
use pangopup_index::{InputAlternative, InputLocus, OrdinaryInputLocus, StreamingIndexWriter};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

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
        let path = std::env::temp_dir().join(format!("pangopup-heap-bound-{}", std::process::id()));
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

#[test]
fn production_spooler_does_not_retain_loci_or_artifact_sized_heap() {
    const GENES: u64 = 3_000;
    const LOCI_PER_GENE: u32 = 1_000;

    let temp = Temp::new();
    let payload = temp.0.join("payload.scratch");
    let output = temp.0.join("scores.pgi");
    let mut writer = StreamingIndexWriter::create(&payload).expect("create streaming writer");
    let baseline_allocated = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline_allocated, Ordering::SeqCst);
    let baseline_rss = resident_bytes();
    let zero = ScoreMagnitude::new(0).expect("zero score");
    let minus_fifty = RelativePosition::new(-50).expect("default position");
    let score = PangolinScore::new(zero, minus_fifty, zero, minus_fifty);

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
        writer.push_gene(&loci).expect("spool one complete gene");
    }

    let scratch_bytes = writer.scratch_bytes();
    assert_eq!(scratch_bytes, GENES * u64::from(LOCI_PER_GENE) * 11);
    let retained_allocated = CURRENT
        .load(Ordering::SeqCst)
        .saturating_sub(baseline_allocated);
    let peak_delta = PEAK
        .load(Ordering::SeqCst)
        .saturating_sub(baseline_allocated);
    let rss_delta = resident_bytes().saturating_sub(baseline_rss);

    assert!(
        retained_allocated < scratch_bytes / 8,
        "retained heap {retained_allocated} is too close to {scratch_bytes} scratch bytes"
    );
    assert!(
        peak_delta < scratch_bytes / 8,
        "peak heap delta {peak_delta} exceeds the one-gene plus compact-state bound"
    );
    #[cfg(target_os = "linux")]
    assert!(
        rss_delta < scratch_bytes / 2,
        "RSS delta {rss_delta} suggests retained locus/artifact state for {scratch_bytes} scratch bytes"
    );

    let summary = writer.finish(&output).expect("finish streaming index");
    assert_eq!(summary.loci, GENES * u64::from(LOCI_PER_GENE));
    assert_eq!(summary.segments, GENES);
    eprintln!(
        "heap-bound scratch_bytes={scratch_bytes} retained_allocated={retained_allocated} peak_delta={peak_delta} rss_delta={rss_delta}"
    );
}
