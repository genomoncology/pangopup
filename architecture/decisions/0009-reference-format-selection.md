# 0009 — Compact GRCh38 reference payload selection

Status: accepted

## Decision

Use the `acgt2-rle-v1` payload—two-bit ACGT plus exact ambiguity runs—as the
only reference encoding eligible for production hardening. Ticket 010 compared
it with uppercase ASCII and exact four-bit IUPAC in the common benchmark-only
`PGRBEN01` container over the pinned six-contig RefSeq GRCh38.p14 input.

The retained five-round result selected `acgt2-rle-v1` by the first, speed
stage. Its headline p50/p95 were 4,469/4,880 ns, compared with 16,272/18,366 ns
for ASCII and 34,267/41,522 ns for IUPAC4. It exceeded the required five-percent
advantage at both quantiles against both candidates. Page, installed-size, and
Zstandard tie-breaks therefore did not participate. The winner used 16 logical
pages versus 22 for ASCII and 14 for IUPAC4; its 165,759,160-byte member and
144,828,782-byte pinned Zstandard frame were nevertheless also the smallest.
Every measured copy reported zero allocations.

The complete retained evidence and identities are in
[`../../planning/artifacts/010-reference-format-selection.md`](../../planning/artifacts/010-reference-format-selection.md).

## Consequences

- The next production reference ticket may harden only `acgt2-rle-v1` into a
  complete 25-primary-sequence bundle and reader.
- Normal tests use a small independent IUPAC oracle and never read the retained
  benchmark input.
- The checked benchmark container remains isolated from the shipped runtime;
  selection does not itself make it a production format or asset.
- Production work must independently specify and prove its manifest, aliases,
  builder, bounds, corruption handling, memory/page behavior, and delivery.
- The retained result is one-host, one-CPU, warm/page-cache evidence over 14
  exact model contexts on six contigs. It makes no cold-I/O, full-genome,
  accelerator, or model-inference performance claim. Reported high RSS includes
  resident file-backed mmap pages touched by exhaustive inspection, not an
  equivalent per-request heap allocation.
