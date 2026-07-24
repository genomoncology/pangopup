use pangopup_build::reference::build_reference_bundle;
use pangopup_core::{GenomicPosition, Grch38Contig, ReferenceProvider};
use pangopup_index::reference::ReferenceBundleOpen;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

struct TrackingAllocator;
static CURRENT: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

fn add(size: usize) {
    let current = CURRENT.fetch_add(size as u64, Ordering::SeqCst) + size as u64;
    PEAK.fetch_max(current, Ordering::SeqCst);
    CALLS.fetch_add(1, Ordering::SeqCst);
    BYTES.fetch_add(size as u64, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegate the unchanged allocation request to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: pointer/layout are the matching allocator pair.
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
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pangopup-reference-resources-{}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale resources test");
        }
        fs::create_dir(&path).expect("create resources test");
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove resources test");
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/reference-production-mini")
        .join(name)
}

#[test]
fn builder_and_reader_heap_are_bounded_and_copy_allocates_nothing() {
    let temp = Temp::new();
    let output = temp.0.join("bundle");
    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    let outcome = build_reference_bundle(
        "pangopup-reference-mini-v1",
        &fixture("source.fa.gz"),
        &fixture("assembly_report.txt"),
        &output,
    )
    .expect("build miniature");
    drop(outcome);
    let builder_peak = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    assert!(
        builder_peak <= 16 * 1024 * 1024,
        "builder peak {builder_peak}"
    );

    let open_baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(open_baseline, Ordering::SeqCst);
    let opened = ReferenceBundleOpen::open(&output).expect("open miniature");
    let open_peak = PEAK.load(Ordering::SeqCst).saturating_sub(open_baseline);
    assert!(open_peak <= 2 * 1024 * 1024, "open peak {open_peak}");

    let mut destination = [0_u8; 15];
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut destination,
        )
        .expect("warm copy");
    CALLS.store(0, Ordering::SeqCst);
    BYTES.store(0, Ordering::SeqCst);
    opened
        .copy_window(
            Grch38Contig::autosome(1).expect("chr1"),
            GenomicPosition::new(1).expect("one"),
            &mut destination,
        )
        .expect("measured copy");
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(BYTES.load(Ordering::SeqCst), 0);
    assert_eq!(&destination, b"ACGTRYSWKMBDHVN");
}
