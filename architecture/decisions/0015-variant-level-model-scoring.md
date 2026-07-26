# 0015 — Variant-level model scoring

Status: accepted

Superseded in part by ADR 0016: routing and post-score stable-gene filtering
now live in `pangopup-engine`; `VariantScorer` itself remains unfiltered.

## Decision

Add one `pangopup-engine` composition crate above the existing core, index, and
raw-model boundaries. Its mutable single-owner `VariantScorer` accepts an owned
literal `Grch38Variant`, fixes GRCh38/masked/distance-50 behavior, and composes
one `ReferenceProvider`, `MaskProvider`, and `ModelKernel`. Lookup routing,
gene filtering, asset discovery/delivery, caching, pooling, CLI output, HTTP,
and CPU policy remain outside this crate.

The supported literal allele shapes are SNVs, equal-length MNVs, and
left-anchored insertions/deletions, with each allele no longer than 100 bases.
The submitted tuple is its identity: no trimming, left alignment, equivalent
representation collapse, HGVS parsing, or transcript projection occurs.

For position `P` and REF length `R`, scoring requests `10,100 + R` reference
bases from one-based `P - 5,050`, compares REF at offset `5,050`, accepts only
A/C/G/T/N reference symbols, constructs the alternate by exact slice
replacement, and queries every containing gene without a filter. Inference
order is plus-reference, plus-alternate, minus-reference, minus-alternate,
skipping an empty strand and stopping at the first failure.

Equal-length and insertion reconciliation remains binary32. Insertions collapse
the center-expanded alternate interval at its first maximum. Deletions insert
binary64 zeroes after center index 50 and retain the resulting binary64
promotion. Each three-replicate tissue group is averaged before first-index
minimum loss and maximum gain are selected across four tissues.

Masking mutates one shared gain/loss pair while visiting authenticated genes in
plus-before-minus and within-strand query order. Gains are clamped at annotated
boundaries and losses away from them; a gene with no boundary facts clamps all
negative loss and receives `NoAnnotatedSites`. Public values use
dtype-local ties-to-even centi rounding, validate gain/loss sign and range,
collapse either signed zero to numeric zero, and retain positions
`-50..=149`.

Ordinary `MaskDomainsOpen::open` stays a cheap structural mmap open. The
separate production-qualification open checks expected size and SHA-256 through
one no-follow regular single-link descriptor, detects pathname replacement,
maps that same descriptor/inode, retains it with the provider, and returns the
verified member identity. This operates within ADR 0013's immutable-inode
contract; concurrent in-place mutation or truncation remains outside the
supported threat model.

## Consequences

- Variant-level model parity is available as a Rust library without changing
  lookup CLI bytes or introducing a routed fallback claim.
- Public results retain exact versioned/PAR GENCODE identity and authenticated
  order rather than sorting by stable gene ID.
- The raw ONNX kernel remains independently testable and unaware of variants,
  masking, or public scoring.
- Normal tests replay checked raw channels and controlled vectors without
  production assets, Python, PyTorch, FASTA, GTF, SQLite, or network access.
- Low-bit differences between the independently generated raw-kernel goldens
  and the older batched compatibility capture are retained honestly: an exact
  digest receipt fixes the post-ensemble arrays derived from the kernel
  goldens, while public masked hundredths, positions, and gene order must match
  the compatibility oracle.
- Lookup-first routing, stable CLI model JSON, complete-request CPU policy,
  coherent asset delivery, caching, and HTTP remain separately reviewable
  outcomes.
