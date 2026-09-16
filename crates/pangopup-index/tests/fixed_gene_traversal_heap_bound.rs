use pangopup_index::{IndexReader, VisitAllError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

struct TrackingAllocator;

static CURRENT: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static MAX_REQUEST: AtomicU64 = AtomicU64::new(0);

fn add(size: usize) {
    let current = CURRENT.fetch_add(size as u64, Ordering::SeqCst) + size as u64;
    PEAK.fetch_max(current, Ordering::SeqCst);
    MAX_REQUEST.fetch_max(size as u64, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Delegate the unchanged allocation request to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: `pointer` and `layout` are the matching allocator pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Delegate the unchanged old pair and requested new size.
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

#[test]
fn fixed_gene_traversal_rejects_before_allocation_and_sort_stays_in_buffer() {
    let scores = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/snv-regression/bundle/scores.pgi");
    let reader = IndexReader::open(&scores).expect("open fixed fixture");

    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    MAX_REQUEST.store(0, Ordering::SeqCst);
    let mut visited = false;
    let rejected = reader.visit_genes_bounded(1, u64::MAX, |_| {
        visited = true;
        Ok::<_, ()>(())
    });
    assert!(!visited);
    assert!(matches!(rejected, Err(VisitAllError::Index(_))));
    assert!(CURRENT.load(Ordering::SeqCst).saturating_sub(baseline) <= 64);
    assert!(PEAK.load(Ordering::SeqCst).saturating_sub(baseline) <= 64);
    assert!(MAX_REQUEST.load(Ordering::SeqCst) <= 64);

    let baseline = CURRENT.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    MAX_REQUEST.store(0, Ordering::SeqCst);
    let summary = reader
        .visit_genes_bounded(3_000_000, 512 * 1024 * 1024, |_| Ok::<_, ()>(()))
        .expect("bounded complete traversal");
    let retained = CURRENT.load(Ordering::SeqCst).saturating_sub(baseline);
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(baseline);
    assert!(retained <= 64);
    assert!(peak <= summary.maximum_buffered_gene_capacity_bytes + 64);
    assert!(MAX_REQUEST.load(Ordering::SeqCst) <= summary.maximum_buffered_gene_capacity_bytes);
    assert!(summary.maximum_buffered_gene_capacity_bytes <= 512 * 1024 * 1024);
    eprintln!(
        "fixed-gene-traversal genes={} loci={} peak_heap={} retained_heap={retained}",
        summary.genes, summary.loci, peak
    );
}
