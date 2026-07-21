# Goals

## Durable outcomes

1. **Exact, gene-aware SNV annotation.** A GRCh38 genomic SNV returns every
   matching source-gene record with the exact published masked gain/loss scores
   and positions and no floating-point drift.
2. **Speed-first selective mmap lookup.** Runtime touches only small directory
   and payload regions; format choice is justified by measured latency and work,
   with installed size treated as secondary.
3. **Reproducible artifacts.** Rust builders stream pinned inputs, prove their
   invariants, write deterministically, certify outputs, and record enough
   provenance to reproduce them.
4. **Standalone typed service core.** CLI and HTTP adapters share one Rust API;
   lookup and model fallback require no external application, database, or
   network service.
5. **Compatible model fallback.** Supported non-SNV variants run through
   versioned model, reference, and masking assets with measured parity against
   the upstream implementation.
6. **Operationally simple delivery.** Immutable executable, lookup, model,
   reference, and masking assets are separately versioned, verified, installed
   atomically, and opened once.
7. **License-complete packaging.** GPL source/model obligations and the score
   dataset's CC BY attribution are explicit, separate, and retained in every
   applicable release artifact.

## Not first-slice goals

- running the Pangolin model;
- serving non-SNVs;
- REST deployment;
- GRCh37/liftover;
- clinical classification thresholds;
- HGVS parsing, transcript/protein projection, and general gene annotation;
- application-level result caching.
