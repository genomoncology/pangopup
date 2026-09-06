use pangopup_core::{EnsemblGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask::{MaskDomainsOpen, MaskProvider, MaskQueryBuffer};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    path::Path,
    str::FromStr,
};

struct CountingAllocator;

std::thread_local! {
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<u64> = const { Cell::new(0) };
}

fn record_allocation() {
    TRACKING.with(|tracking| {
        if tracking.get() {
            CALLS.with(|calls| calls.set(calls.get().checked_add(1).expect("allocation count")));
        }
    });
}

fn measure_allocations<T>(operation: impl FnOnce() -> T) -> (T, u64) {
    CALLS.with(|calls| calls.set(0));
    TRACKING.with(|tracking| assert!(!tracking.replace(true), "measurement already active"));
    let result = operation();
    TRACKING.with(|tracking| assert!(tracking.replace(false), "measurement became inactive"));
    let calls = CALLS.with(Cell::get);
    (result, calls)
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegate the unchanged allocation request to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation();
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pointer/layout are the matching allocator pair.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: delegate the unchanged old pair and requested new size.
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            record_allocation();
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn sufficiently_reserved_warmed_queries_allocate_nothing() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gencode-mask-mini/domains.pgm");
    let provider = MaskDomainsOpen::open(&path).expect("open member");
    let mut output = MaskQueryBuffer::with_capacity(4, 8);
    let contig = Grch38Contig::autosome(1).expect("chr1");
    let position = GenomicPosition::new(2).expect("position");
    let stable = EnsemblGeneId::from_str("ENSG00000000001").expect("stable gene");
    provider
        .query(contig, position, Some(stable), &mut output)
        .expect("warm query");

    let (_, calls) = measure_allocations(|| {
        for _ in 0..10_000 {
            provider
                .query(contig, position, Some(stable), &mut output)
                .expect("measured query");
        }
    });
    assert_eq!(calls, 0, "warmed query allocated");
    assert_eq!(output.plus().len(), 1);
    assert!(output.minus().is_empty());
}

#[test]
fn unrelated_thread_allocation_does_not_change_measurement() {
    use std::sync::{Arc, Barrier};

    let start = Arc::new(Barrier::new(2));
    let finish = Arc::new(Barrier::new(2));
    let worker_start = Arc::clone(&start);
    let worker_finish = Arc::clone(&finish);
    let worker = std::thread::spawn(move || {
        worker_start.wait();
        std::hint::black_box(Box::new(42_u64));
        worker_finish.wait();
    });
    let (_, calls) = measure_allocations(|| {
        start.wait();
        finish.wait();
    });
    worker.join().expect("unrelated allocator thread");
    assert_eq!(calls, 0, "unrelated thread contaminated measurement");
}

#[test]
fn measured_thread_allocation_is_detected() {
    let (allocation, calls) = measure_allocations(|| std::hint::black_box(Box::new(42_u64)));
    assert!(calls > 0, "intentional allocation was not counted");
    drop(allocation);
}
