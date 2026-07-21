# Overlapping-gene mask order

Status: open

## Observation

Upstream `process_variant` computes one pair of gain/loss arrays for each strand,
then loops through genes on that strand. Masking changes those shared arrays in
place. With two overlapping same-strand genes, the second result can therefore
include masking already applied for the first gene and depend on annotation
iteration order.

## Why it matters

The sparse SNV index is unaffected because its published values are the source
truth. Model fallback can diverge from upstream if a Rust implementation makes
a fresh copy per gene, and can remain accidentally order-dependent if it does
not.

## Required evidence before model implementation

- Construct an overlapping same-strand fixture with different exon boundaries.
- Record current Python output and annotation iteration order.
- Determine whether the published precompute contains observable examples.
- Version the chosen behavior. Prefer exact upstream compatibility for a
  compatibility provider; any corrected independent-gene policy must be named
  differently and must not claim byte-for-byte Pangolin parity.
