# 0004 — Speed-first runtime and release assets

Status: accepted
Date: 2026-07-21

## Decision

After correctness, the optimization order is query performance, resident memory
and pages touched, then compressed download size. The hierarchical sparse,
decompression-free mmap representation is the v1 baseline. Benchmarks must
quantify it against fixed-width and independently compressed layouts, but a
smaller file alone does not displace it.

Large generated data and model artifacts are delivered through GitHub Releases,
not Git or Git LFS. They may be compressed for transport and are expanded and
verified once during automatic or explicit installation. The request path never
downloads or decompresses an SNV lookup.

## Consequences

- The operating-system page cache remains the normal hot cache.
- The compressed transport asset must stay under the hosting limit or transport
  is split by contig without changing the installed lookup format or semantics.
- Data, model, reference, masking, and executable assets have separate
  identities and notices.
- Installation is automatic by default, available explicitly for prefetching,
  and atomic; the core scoring library itself only opens paths.
