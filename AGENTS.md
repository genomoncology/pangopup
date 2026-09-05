# AGENTS.md — Pangopup

Pangopup is a GPL Rust workspace for a standalone Pangolin-compatible splice
service. The target product combines exact lookup of published GRCh38 SNV
scores with model fallback for lookup misses and supported non-SNVs. The
repository currently ships the source inspector, deterministic complete-corpus
builder and verifier, fixed 11-byte mmap reader, typed score-provider API,
batch lookup CLI, Linux and macOS local-user asset installation/discovery, the immutable
public `snv-grch38-v1` lookup-data release, pinned resumable remote sync, and a
strict frozen upstream compatibility corpus with a bounded offline inspector,
the qualified compiled RefSeq GRCh38.p14 sequence-index bundle/provider, and an
authenticated ONNX Runtime CPU kernel returning twelve raw Pangolin channels.
The `pangopup-engine` crate now composes those providers into compatible
variant-level scoring and lookup-first routing for the supported literal
allele subset. The CLI consumes the activated installed runtime profile lazily
for fallback, while retaining a complete explicit reference/mask/model
override. Its `--model-only` flag bypasses SNV lookup through a distinct typed
engine request and can use either the activated model-side profile or a
self-sufficient explicit model tuple without opening an SNV asset. The CLI
emits exact modeled JSONL/table results. Complete-request CPU policy
selection is established. A retained ten-candidate service-partition
experiment selects host-qualified `1×1`, `1×2`, `1×4`, and `2×4` mappings on
the retained Ryzen host while preserving portable `1×1` and keeping production
service mappings. The foreground HTTP service now keeps lookup/cache hits
outside one fixed bounded FIFO model queue and exposes health/status. Ticket
022 corrected and repeated the
singleton/zero-padded/paired comparison after code review caught missing v2
export axes. Both policies were inconclusive from singleton drift and neither
candidate met the independent replacement gates, so ordinary dispatch remains
singleton. Ticket 023 adds persistent exact SQLite reuse for successful
complete model results while preserving lookup-first laziness. One canonical
four-asset runtime profile now binds the exact compatible production tuple;
offline Linux/macOS XDG installation, atomic activation, and lookup consumption are
shipped. Deterministic local model-side packaging, immutable publication, and
the typed pinned model-side sync/install primitive and combined top-level CLI
provisioning/status are shipped. The immutable Linux x86_64 executable and its
checksum-verifying tagged installer are shipped; the deterministic six-file
release preparer and read-only exact-commit packaging workflow remain available.
The thin non-root native AMD64/ARM64 Docker image is shipped through one public
GHCR index without embedded assets; process-manager packaging remains future
work. The
reference
wire/writer and sole mmap reader are now separate; future reference builds use
v2 byte-producing provenance and installed admission has one held-descriptor
reader boundary. A
reviewed retained benchmark selected the
two-bit/ambiguity-run GRCh38 payload by speed; Ticket 011 hardened it as the
production `PGRREF01` reader and qualified the complete 25-contig bundle. Read
`README.md` first.
Ticket 014 now exposes Ticket 012's selected byte-identical GENCODE v38
`domains` member through the production `pangopup_index::mask` mmap provider;
the one-time candidate/qualification source has been removed while its retained
selection evidence remains. Compiled sequence-index, mask, and model local
packaging, immutable publication, pinned sync, and direct one-pass install from
cached transport and combined CLI provisioning/status are shipped.

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
  deterministic writing, offline certification, and compatibility-corpus
  capture/inspection. It also owns the authenticated maintainer-only model
  evidence/conversion adapters. The checked corpus and miniature model are
  replayed offline; normal gates must never invoke expensive Python capture or
  conversion paths. It also composes the production-only four-asset profile
  without initializing ONNX. Builder-only dependencies must not enter runtime
  consumers.
- `crates/pangopup-assets` owns the strict path-free runtime-profile grammar,
  production tuple trust check, and bounded SNV metadata admission in addition
  to SNV delivery.
- `crates/pangopup-model` owns the exact selected v1 singleton contract,
  context/strand encoding, and one mutable CPU ONNX Runtime session returning
  twelve raw channels. Closed v2 contracts and bounded candidate execution are
  retained only for maintainer reproduction. It does not own genomic variant
  construction, post-processing, masking, routing, or a public score provider.
- `crates/pangopup-engine` owns pre-canonical exact-edit values, conversion to the shared canonical literal variant, fixed GRCh38 distance-50 variant construction,
  ensemble/indel arithmetic, order-sensitive masking, extrema, exact public
  modeled results, and the small lookup-first router/filter boundary. It does
  not own transport, caching, CLI, asset-opening policy, or concurrency policy.
- `crates/pangopup-cache` owns the disposable versioned SQLite schema, complete
  scoring-identity keys, canonical typed values, insertion/update-order bounds,
  and safe recovery. It owns no scoring, filtering, asset delivery, or service
  policy.
- `crates/pangopup-cli` adapts command-line strings and output to the typed API;
  it composes cache admission with fallback but contains no scoring or index
  logic.
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

This repository carries an `sdlc/` folder and is onboarded to the sdlc factory.
`sdlc/project/` holds the five scripts the factory calls, copied verbatim from
the canonical set and never hand-edited. `sdlc/scripts/` holds this project's
own `install`, `lint`, `test` and `spec`, which run the gates above.

- Drafts live in `sdlc/tickets/drafts/`. Sync never sees them.
- A top-level ticket on `origin/main` is approved and will be dispatched
  unattended. Promotion is approval. Make the human decision before you move
  the file, not after.
- Tickets carry `flow` and `priority` frontmatter and are numbered
  `NNNN-slug.md` on one counter shared with `sdlc/records/` and
  `sdlc/tickets/archive/`. Never choose that number by hand. Run `file-ticket`
  (or `next-id` for a draft) from inside the repo.
- Sync reads `origin/main`, never a working tree. An unpushed ticket does not
  exist.
- A completed ticket gets a matching record in `sdlc/records/`. The ticket file
  moves to `sdlc/tickets/archive/`.
- Keep tickets short. One behavior, why it matters, observable acceptance
  criteria. Never add a changing status field. Never write the design into the
  ticket. The design stage owns it.
- Never start a manual bot run while a factory attempt is live.

A ticket must identify its observable acceptance test, its inside-out tests,
and the performance or size evidence required for a format-sensitive change.
Do not freeze a byte layout from intuition. First pin a checked-in miniature
source fixture, then compare candidate layouts using the same queries and
exactness corpus. Preserve source attribution and provenance in every produced
bundle.

Documentation is part of the implementation, not cleanup. A ticket names the
durable and user-facing documents its outcome changes.

`planning/` holds the history of the coordinator-and-sub-agent workflow this
repository used before the factory. Read it as a record. Do not file new work
there.
