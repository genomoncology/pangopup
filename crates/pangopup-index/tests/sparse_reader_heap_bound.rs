use pangopup_core::{
    DnaBase, EnsemblGeneId, GenomicPosition, Grch38Contig, PangolinScore, RelativePosition,
    ScoreMagnitude,
};
use pangopup_index::{
    InputAlternative, InputLocus, OrdinaryInputLocus, sparse_reader::SparseIndexReader,
    sparse_writer::SparseIndexWriter,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    path::{Path, PathBuf},
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
        // SAFETY: Delegate the unchanged layout to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: `pointer` and `layout` are the matching allocator pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Delegate the unchanged old pair and new size.
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
        let path = std::env::temp_dir().join(format!(
            "pangopup-sparse-reader-heap-bound-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("create heap test directory");
        Self(path)
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove heap test directory");
    }
}

#[test]
fn reader_heap_stays_bounded_as_locus_count_grows() {
    let temp = Temp::new();
    let small = build(&temp.0, "small", 64);
    let large = build(&temp.0, "large", 50_000);
    let small_measurement = measure(&small, 64);
    let large_measurement = measure(&large, 50_000);
    for (label, (peak, retained)) in [("small", small_measurement), ("large", large_measurement)] {
        assert!(
            peak <= 1024 * 1024,
            "{label} reader added {peak} peak heap bytes"
        );
        assert!(
            retained <= 64 * 1024,
            "{label} reader retained {retained} heap bytes after traversal"
        );
    }
    eprintln!(
        "sparse-reader-heap-bound small_peak={} small_retained={} large_peak={} large_retained={}",
        small_measurement.0, small_measurement.1, large_measurement.0, large_measurement.1
    );
}

fn measure(path: &Path, expected_loci: u64) -> (u64, u64) {
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    let reader = SparseIndexReader::open(path).expect("open measured reader");
    let summary = reader
        .visit_all(|_| Ok::<_, ()>(()))
        .expect("stream measured reader");
    assert_eq!(summary.loci, expected_loci);
    let retained = CURRENT.load(Ordering::SeqCst).saturating_sub(baseline);
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    drop(reader);
    (peak, retained)
}

fn build(directory: &Path, label: &str, loci: u32) -> PathBuf {
    let scratch = directory.join(format!("{label}.scratch"));
    let output = directory.join(format!("{label}.pgi"));
    let gene = EnsemblGeneId::from_numeric(1).expect("gene");
    let contig = Grch38Contig::from_code(1).expect("contig");
    let score = PangolinScore::new(
        ScoreMagnitude::new(1).expect("gain"),
        RelativePosition::new(-49).expect("gain position"),
        ScoreMagnitude::new(2).expect("loss"),
        RelativePosition::new(49).expect("loss position"),
    );
    let input: Vec<_> = (1..=loci)
        .map(|coordinate| {
            InputLocus::Ordinary(OrdinaryInputLocus {
                gene,
                contig,
                position: GenomicPosition::new(coordinate).expect("coordinate"),
                reference: DnaBase::A,
                alternatives: [DnaBase::C, DnaBase::G, DnaBase::T]
                    .map(|alternate| InputAlternative { alternate, score }),
            })
        })
        .collect();
    let mut writer = SparseIndexWriter::create(&scratch).expect("writer");
    writer.push_gene(&input).expect("gene");
    writer.finish(&output).expect("finish candidate");
    output
}
