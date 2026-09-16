# 0004 — Speed-first runtime and release assets

Status: optimization order superseded by ADR 0027 for the next SNV format; delivery principles retained
Date: 2026-07-21

Supersession note (2026-09-04): Ticket 0003 extended the shipped local installer to native macOS. The original Linux installer sentence below remains as history.

Supersession note (2026-09-15): ADR 0027 replaces the speed-first optimization order for selection of the next SNV format with predeclared correctness, installed `scores.pgi` size, and bounded latency gates. Fixed-v1 and the delivery principles below remain unchanged.

## Decision

After correctness, the optimization order is query performance, resident memory
and pages touched, then compressed download size. This decision originally made
the hierarchical sparse mmap representation a provisional baseline pending
measurement. ADR 0006 superseded that format choice: the measured fixed 11-byte
layout is the only supported private v1 format, and sparse layouts remain
historical candidates. That optimization order governed the fixed-v1
selection. ADR 0027 supersedes it for selection of the next SNV format.

Large generated data and model artifacts are intended for delivery through
GitHub Releases, not Git or Git LFS. The shipped local SNV transport carries
a canonical manifest, exact copies of the small installed members, and one
deterministic compressed `scores.pgi` stream cut into ordered exact
1,000,000,000-byte parts except for its final part. Explicit local unpack
verifies and reconstructs the unchanged fixed-v1 bundle once. The shipped Linux
installer streams that transport into an immutable XDG-data bundle, atomically
selects it, and reuses it with cheap structural validation. The request path
never downloads or decompresses an SNV lookup.

## Consequences

- The operating-system page cache remains the normal hot cache.
- Each score-stream part stays below the hosting limit without splitting the
  installed index by contig or changing its format or semantics.
- Data, model, reference, masking, and executable assets have separate
  identities and notices.
- The target installation flow is automatic by default, available explicitly
  for prefetching, and atomic; the shipped core scoring library only opens
  paths.
