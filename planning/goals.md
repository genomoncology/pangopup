# Goals

## Durable outcomes

1. **Exact SNV annotation.** A normalized GRCh38 genomic SNV returns the same
   numeric masked gain/loss scores and positions as the pinned source row, with
   no floating-point drift.
2. **Gene-aware truth.** Overlapping genes and gene-specific masking remain
   explicit. Pangopup never collapses several source records into an unexplained
   single answer.
3. **Selective mmap lookup.** Runtime work touches only small directory and
   payload regions; it never scans or parses the full source at query time.
4. **Speed-first measured representation.** The chosen format minimizes lookup
   work and latency, with size reported against fixed records, block compression,
   and a Tabix baseline rather than treated as the primary objective.
5. **Reproducible artifact.** A Rust builder streams the pinned source, proves
   its invariants, writes deterministically, certifies the result, and records
   enough provenance to reproduce it.
6. **Typed reusable library.** CLI and future service adapters share one Rust
   score-provider contract while storage and transport details stay private.
7. **Operationally simple delivery.** One immutable bundle opens once, fails
   closed if incompatible or corrupt, works concurrently, and relies first on
   the operating-system page cache.
8. **License-complete packaging.** GPL source delivery and the source dataset’s
   CC BY attribution are explicit, separate, and retained in derived bundles.
9. **Standalone deployment.** Lookup and model fallback require no Genome, UTA,
   SeqRepo, database, or network service.
10. **Release-asset delivery.** Executables, lookup data, and models are
    separately versioned, verified assets whose transport encoding never enters
    the query path.

## Not first-slice goals

- running the Pangolin model;
- serving non-SNVs;
- REST deployment;
- GRCh37/liftover;
- clinical classification thresholds;
- a general HGVS implementation;
- application-level result caching.
