# 0004 — Speed-first runtime and release assets

Status: accepted
Date: 2026-07-21

## Decision

Warm and cold lookup speed are more important than installed or download size.
The hierarchical sparse, decompression-free mmap representation is the v1
baseline. Benchmarks must quantify it against fixed-width and independently
compressed layouts, but a smaller file alone does not displace it.

Large generated data and model artifacts are delivered through GitHub Releases,
not Git or Git LFS. They may be compressed for transport and are expanded and
verified once during explicit installation. Runtime never downloads or
decompresses an SNV lookup.

## Consequences

- The operating-system page cache remains the normal hot cache.
- The final direct bundle must stay under the hosting limit or transport is
  split by contig without changing lookup semantics.
- Data, model, and executable assets have separate identities and notices.
- Installation is explicit and atomic; the library itself only opens paths.
