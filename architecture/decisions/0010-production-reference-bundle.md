# 0010 — Production GRCh38 reference bundle

Status: accepted

## Decision

Harden ADR 0009's `acgt2-rle-v1` payload as the distinct `PGRREF01` v1 bundle
and expose it through the typed, caller-buffer `ReferenceProvider`. Production
contains exactly the 25 RefSeq GRCh38.p14 assembled molecules chr1–22, X, Y,
and non-nuclear M. The original FASTA and assembly report are authenticated
build inputs, not runtime dependencies.

Runtime open validates bounded structure and the complete ambiguity table but
does not hash or traverse dense bytes. Private build certification owns full
member hashing, canonical-padding checks, complete decoded logical identity,
and compatibility-context replay. Do not add page checksums without evidence:
query speed remains the first priority, followed by heap use and download size.

## Consequences

- The benchmark `PGRBEN01` files remain incompatible and cannot be installed as
  production references.
- One long-lived mmap serves exact uppercase-IUPAC copies with zero per-copy
  heap allocation.
- Bundle transport, installation, and publication are not implied by the
  format and receive separate review.
- Model admission must require the exact production profile, format, assembly
  accession, and logical sequence digest; the synthetic profile is test-only.
- Valid dense-bit corruption is outside cheap-open detection. Build
  certification catches it, and future installation must verify member hashes
  before activation.
