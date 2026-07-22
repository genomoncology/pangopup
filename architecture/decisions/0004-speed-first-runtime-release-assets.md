# 0004 — Speed-first runtime and release assets

Status: superseded by ADR 0006 for index-format selection; delivery principles retained
Date: 2026-07-21

## Decision

After correctness, the optimization order is query performance, resident memory
and pages touched, then compressed download size. This decision originally made
the hierarchical sparse mmap representation a provisional baseline pending
measurement. ADR 0006 superseded that format choice: the measured fixed 11-byte
layout is the only supported private v1 format, and sparse layouts remain
historical candidates. The optimization order remains in force.

Large generated data and model artifacts are intended for delivery through
GitHub Releases, not Git or Git LFS. They may be compressed and split for
transport and will be reassembled, expanded, and verified once during future
automatic or explicit installation. The asset manager is not implemented. The
request path never downloads or decompresses an SNV lookup.

## Consequences

- The operating-system page cache remains the normal hot cache.
- The compressed transport asset must stay under the hosting limit or transport
  is split by contig without changing the installed lookup format or semantics.
- Data, model, reference, masking, and executable assets have separate
  identities and notices.
- The target installation flow is automatic by default, available explicitly
  for prefetching, and atomic; the shipped core scoring library only opens
  paths.
