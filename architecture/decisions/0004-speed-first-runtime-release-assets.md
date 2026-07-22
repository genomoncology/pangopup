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
GitHub Releases, not Git or Git LFS. The shipped local SNV transport carries
a canonical manifest, exact copies of the small installed members, and one
deterministic compressed `scores.pgi` stream cut into ordered exact
1,000,000,000-byte parts except for its final part. Explicit local unpack
verifies and reconstructs the unchanged fixed-v1 bundle once. The managed asset
installer is not implemented. The request path never downloads or decompresses
an SNV lookup.

## Consequences

- The operating-system page cache remains the normal hot cache.
- Each score-stream part stays below the hosting limit without splitting the
  installed index by contig or changing its format or semantics.
- Data, model, reference, masking, and executable assets have separate
  identities and notices.
- The target installation flow is automatic by default, available explicitly
  for prefetching, and atomic; the shipped core scoring library only opens
  paths.
