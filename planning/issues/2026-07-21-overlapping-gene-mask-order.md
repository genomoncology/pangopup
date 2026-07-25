# Overlapping-gene mask order

Status: closed by Ticket 012 on 2026-07-25

## Observation

Upstream `process_variant` computes one pair of gain/loss arrays for each strand,
then loops through genes on that strand. Masking changes those shared arrays in
place. With two overlapping same-strand genes, the second result can therefore
include masking already applied for the first gene and depend on annotation
iteration order.

## Why it matters

The precomputed fixed-v1 SNV lookup is unaffected because its published values
are the source truth. Model fallback can diverge from upstream if a Rust
implementation makes a fresh copy per gene, and can remain accidentally
order-dependent if it does not.

## Evidence already established

Ticket 009 constructed a controlled same-strand overlap vector, captured the
pinned Python behavior and database order, and accepted the strict behavior as
profile `pangolin-1.0.2-5cf94b8-grch38-v1` in ADR 0008. A corrected independent-
gene policy still requires a separately named profile.

That evidence proves the behavior but not the complete production data. The
retained compatibility artifact explicitly says it does not contain an
all-gene masking-order representation.

## Contract completed by Ticket 012

- Inventory every same-strand overlap and every distinct effective
  `(gene_start,gene_end]` point-query gene order in the exact pinned GENCODE v38
  gffutils database.
- Because the upstream query has no SQL `ORDER BY`, authenticate and record the
  Python/gffutils/SQLite observation environment and freeze a canonical ordered
  export rather than treating the database hash alone as an ordering contract.
- Define the versioned gene identity and ordered logical stream that a compact
  mask member must preserve.
- Prove the compiled candidates against the database over the complete logical
  domain, not only selected examples.
- Retain a full-source digest and overlap/order summary so later production
  hardening cannot silently sort genes by identifier or coordinate.

Ticket 012 completed this contract over the exact pinned GENCODE v38 source:
60,649 ordered exact genes, 88,202 effective domains, every domain witness and
edge, and all three compiled candidates were authenticated and compared. ADR
0011 selected constant-membership domains for production hardening. Preserving
the resulting order remains a production-format and model-parity invariant,
not an open issue or second implementation queue.
