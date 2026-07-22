# 0006 — Fixed 11-byte locus index selected by measurement

Status: accepted
Date: 2026-07-21

## Decision

Pangopup's private v1 prototype index uses one 11-byte fixed record per ordinary
locus. The fixed candidate displaced the hierarchical sparse baseline because
it won the accepted speed-first priority on every primary warm workload,
including reopen-plus-query. Its larger installed size is accepted; size cannot
overrule a clear query-speed win under ADR 0004.

Tabix did not trigger the no-custom-format stop branch. Its fair in-process
one-open single-record p50 was 9.372 ms on the retained lab corpus, compared
with 121 ns for the equal-harness fixed kernel and 210 ns for the hardened
selected reader.

Only fixed-11 is promoted to product writer/reader code. Hierarchical direct,
the independent fixed kernel, Zstd/LZ4 sparse blocks at 1,024, 2,048, and 4,096
loci, and Tabix remain benchmark-only implementations.

## Private v1 invariants

- Integers and packed bits have explicitly named little-endian order. Mapped
  bytes are never cast to Rust structs.
- The fixed header declares magic, version, exact file length, nonoverlapping
  ordered section ranges and counts, plus 25 per-contig tree roots.
- Segment entries are sorted by Ensembl gene numeric suffix, contig code, and
  ascending segment start. A segment is contiguous; `end - start + 1` equals
  its locus count. Gaps and `REF=N` positions split ordinary segments.
- Every contig has a balanced interval tree over its segments. Each node stores
  its subtree's maximum end. An unfiltered point query prunes by start and
  maximum end and runs in `O(log n + k)` for `k` returned overlaps. A filtered
  query upper-bound-searches the `(gene, contig, start)` segment key and checks
  its sole possible predecessor; it never scans every segment for a gene.
- One ordinary locus occupies exactly 87 meaningful bits in 11 bytes. Bits 0–2
  encode the concrete reference (`A=0, C=1, G=2, T=3`; 4–7 are invalid). The
  other three bases follow in ascending base-code order. Each alternate owns a
  28-bit score: 14-bit gain pair followed by 14-bit loss pair. A pair contains
  seven score-magnitude bits and seven `relative_position + 50` bits; both codes
  must be in 0…100. Bit 87 is reserved and must be zero.
- `REF=N` loci are not ordinary SNVs. Sorted fixed exception records preserve
  gene, contig, position, omitted base, all three published alternate codes,
  and all exact scores.
- Open validates header, section arithmetic, directory order and bounds, the
  small exception directory, and a canonical tree traversal: acyclicity,
  unique connectivity and complete coverage, strict BST ordering, exact
  subtree maxima, contig ownership, and height balance. It does not scan
  ordinary payload. Lookup validates the exact 11-byte record it touches,
  including reference, reserved bit, and all six score/position pairs before
  selecting an alternate. Corruption in untouched ordinary records remains
  lazy and is caught by explicit offline verification.
- Every count, offset, section end, and count × width calculation uses checked
  arithmetic before conversion or byte access.

The only unsafe operation is mmap creation. Its contract requires an immutable
inode that is never modified or truncated while mapped. All later accesses are
bounds-checked explicit decodes. Publication must replace a verified inode
atomically rather than mutate one in place.

## Evidence and alternatives

The deterministic 134-gene lab corpus contains 9,858,991 loci. The promoted
fixed artifact was 108,467,071 bytes, while hierarchical direct was 13,844,635
bytes. After repairing direct to use zero-copy mmap decoding, packed six-bit
masks, and 64-locus rank checkpoints, the equal-harness fixed kernel still won:
warm one-open p50 for 1 / 10 / 100 exact records was 121 / 972 / 9,949 ns for
fixed and 160 / 1,243 / 14,749 ns for direct. Reopen p50 was 16,533 / 16,583 /
27,805 ns for fixed and 24,208 / 24,970 / 35,660 ns for direct. The separately
hardened selected reader measured 210 / 1,964 / 19,588 ns one-open and 20,570 /
22,094 / 39,588 ns reopen.

- Hierarchical direct is rejected for v1 because it lost query and reopen
  performance, despite much smaller size and fewer mapped pages.
- Zstd and LZ4 sparse blocks are rejected because decompression and allocation
  made them slower. Raw fallback remains benchmarked when compression expands.
- Tabix is rejected as product format because in-process indexed queries and
  row parsing were orders of magnitude slower.
- No product block-compression format was selected, so corrupt compressed-block
  length and incomplete-decompression contracts are inapplicable to v1. Those
  benchmark-only codecs do not acquire a production corrupt-input API.

Full measurements, method, hardware, manifests, and cold-I/O limits are in
[`../../planning/artifacts/002-index-format-benchmark.md`](../../planning/artifacts/002-index-format-benchmark.md).

## Consequences

The complete certified member is 15,033,158,255 bytes (about 14.0 GiB), including
directories and exceptions. Ticket 004 retained complete-artifact open, lookup,
CLI, and serialization evidence while leaving cold performance explicitly
unmeasured because this host could not prove nonresidency. Release transport
must be compressed and split into host-compatible members if necessary; this
must not add decompression to a query. Any future format change requires a new
ADR rather than making v1 accept multiple layouts.
