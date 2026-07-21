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
4. **Small, measured representation.** The chosen format sits on the useful
   size/latency Pareto frontier against simpler fixed records, block compression,
   and a Tabix baseline.
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

## Not first-slice goals

- running the Pangolin model;
- serving non-SNVs;
- REST deployment;
- GRCh37/liftover;
- clinical classification thresholds;
- a general HGVS implementation;
- application-level result caching.
