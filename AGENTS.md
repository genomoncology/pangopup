# AGENTS.md — Pangopup

Pangopup is a GPL Rust workspace for a standalone Pangolin-compatible splice
service. The target product combines exact lookup of published GRCh38 SNV scores
with model fallback for supported non-SNVs. The repository currently contains a
walking skeleton and retained source-analysis evidence; the builder, mmap
reader, score API, model runtime, and service are not implemented yet. Read
`README.md` first.

## Repository contract shape

- Observable CLI behavior lives in executable `spec/*.md` documents and runs
  through `make spec`.
- Library behavior, file validation, round trips, and error paths live in Rust
  unit and integration tests and run through `make test`.
- Full-source build and benchmark evidence belongs in `planning/artifacts/`;
  the downloaded source dataset is never committed.

## The gates

```text
make lint = cargo fmt --check + clippy --all-targets with warnings denied
make test = cargo test across the workspace
make spec = build the current CLI + execute spec/*.md with mustmatch
```

There is no `make check`. Run all three gates before committing.

## Layout and conventions

- `crates/pangopup-core` owns public newtypes, score records, lookup results,
  provider traits, and typed errors. It knows no file format or transport.
- `crates/pangopup-index` owns the private format codec, cheap open-time
  structural validation, mmap lifecycle, checked byte decoding, and lookup.
- `crates/pangopup-build` owns gzip/TSV ingestion, full-source validation,
  deterministic writing, and offline certification. Builder-only dependencies
  must not enter runtime consumers.
- `crates/pangopup-cli` adapts command-line strings and output to the typed API;
  it contains no scoring or index logic.
- `architecture/` records durable boundaries and accepted decisions.
- `planning/` is the single source of truth for unfinished work.
- Unsafe mmap setup must remain confined to `pangopup-index`; mapped bytes are
  not used until cheap header/section/source checks pass. Lookups validate bytes
  they touch; offline certification owns payload-wide ordering and count checks
  so runtime open does not page through the whole artifact.
- Keep GRCh38, chromosome/accession, 1-based position, alleles, Ensembl gene ID,
  centi-score, and relative score position as distinct Rust types. Raw strings
  and primitive integers stop at adapters.
- Optimize measured query paths. Mmap and the operating-system page cache are
  the baseline; add application caches or block compression only with evidence.
- Shipped code is Rust. One-off source exploration may use `uv` scripts, but the
  reproducible builder and verifier belong in Rust.

Expected implementation skills are `rust-standards`, `rust-perf-review`,
`mustmatch`, and `testing-mindset`; agent skill links are local tooling and are
not committed.

## How work arrives

Work comes through one bounded file in `planning/tickets/` at a time. A ticket
must identify its observable acceptance test, its inside-out tests, and the
performance or size evidence required for a format-sensitive change. Do not
freeze a byte layout from intuition: first pin a checked-in miniature source
fixture, then compare candidate layouts using the same queries and exactness
corpus. Preserve source attribution and provenance in every produced bundle.

After implementation and focused tests, use an independent read-only review for
format safety, exactness, unnecessary allocation, accidental full scans, and
scope creep before running the final gates and committing.
