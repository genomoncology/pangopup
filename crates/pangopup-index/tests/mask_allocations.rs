use pangopup_core::{EnsemblGeneId, GenomicPosition, Grch38Contig};
use pangopup_index::mask::{MaskDomainsOpen, MaskProvider, MaskQueryBuffer};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    path::Path,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

struct CountingAllocator;
static CALLS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: delegate the unchanged allocation request to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            CALLS.fetch_add(1, Ordering::SeqCst);
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
            CALLS.fetch_add(1, Ordering::SeqCst);
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

    let before = CALLS.load(Ordering::SeqCst);
    for _ in 0..10_000 {
        provider
            .query(contig, position, Some(stable), &mut output)
            .expect("measured query");
    }
    let after = CALLS.load(Ordering::SeqCst);
    assert_eq!(after, before, "warmed query allocated");
    assert_eq!(output.plus().len(), 1);
    assert!(output.minus().is_empty());
}
