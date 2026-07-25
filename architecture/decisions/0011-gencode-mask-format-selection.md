# 0011 — Compact GENCODE mask representation selection

Status: accepted

## Decision

Use constant-membership `domains` as the only GENCODE mask representation
eligible for production hardening. Ticket 012 compared it with a direct
interval-tree baseline and binned postings in the common private, benchmark-
only `PGMBEN01` family over the complete pinned GENCODE v38 logical source.

Every candidate first passed independent complete-domain and edge
certification, corruption controls, bounded open, and zero-allocation warmed
lookup. In the balanced six-round comparison, `domains` reported headline
p50/p95 of 171/331 ns, versus 241/401 ns for `interval-tree` and 241/431 ns
for `binned-postings`. The closed selector retained only `domains` at its first
p95-within-five-percent step. Later page, heap, member-size, compressed-size,
and simplicity criteria did not participate.

The complete retained evidence and identities are in
[`../../planning/artifacts/012-gencode-mask-format-selection.md`](../../planning/artifacts/012-gencode-mask-format-selection.md).

## Consequences

- The next production mask ticket may harden only the domain representation.
- `PGMBEN01` remains an incompatible private qualification format. Selection
  does not create a production magic, manifest, bundle, provider, installer,
  transport, or release asset.
- Production must re-specify the layout under a distinct format identity and
  preserve exact versioned and `_PAR_Y` gene IDs, `(start,end]` membership,
  upstream strand-local order, and normalized exon boundaries.
- Production open and lookup must remain bounded, checked, mmap-backed, and
  allocation-free after callers provide result storage. Build certification
  must independently prove all 25 primary contigs and the compatibility cases.
- The result is one-host, one-CPU, warm/page-cache evidence. It makes no
  cold-I/O, model-inference, accelerator, HTTP, or deployment claim.
- The selected candidate is faster but larger than both alternatives in this
  run. Download size was a later tie-break and did not override the accepted
  speed-first priority.
