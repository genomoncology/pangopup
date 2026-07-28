# AGENTS.md — Pangopup

Pangopup is a GPL Rust workspace for a standalone Pangolin-compatible splice
service. The target product combines exact lookup of published GRCh38 SNV
scores with model fallback for lookup misses and supported non-SNVs. The
repository currently ships the source inspector, deterministic complete-corpus
builder and verifier, fixed 11-byte mmap reader, typed score-provider API,
batch lookup CLI, Linux local-user asset installation/discovery, the immutable
public `snv-grch38-v1` lookup-data release, pinned resumable remote sync, and a
strict frozen upstream compatibility corpus with a bounded offline inspector,
the qualified compiled RefSeq GRCh38.p14 sequence-index bundle/provider, and an
authenticated ONNX Runtime CPU kernel returning twelve raw Pangolin channels.
The `pangopup-engine` crate now composes those providers into compatible
variant-level scoring and lookup-first routing for the supported literal
allele subset. The CLI consumes the activated installed runtime profile lazily
for fallback, while retaining a complete explicit reference/mask/model
override, and emits exact modeled JSONL/table results. Complete-request CPU policy
selection is established. Ticket 022 corrected and repeated the
singleton/zero-padded/paired comparison after code review caught missing v2
export axes. Both policies were inconclusive from singleton drift and neither
candidate met the independent replacement gates, so ordinary dispatch remains
singleton. Ticket 023 adds persistent exact SQLite reuse for successful
complete model results while preserving lookup-first laziness. One canonical
four-asset runtime profile now binds the exact compatible production tuple;
offline Linux/XDG installation, atomic activation, and lookup consumption are
shipped. Model-side delivery and the HTTP service remain target work. The reference
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
selection evidence remains. Compiled sequence-index, mask, and model delivery
remain future work.

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
- `crates/pangopup-engine` owns fixed GRCh38 distance-50 variant construction,
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

Work comes through one bounded file in `planning/tickets/` at a time. A ticket
must identify its observable acceptance test, its inside-out tests, and the
performance or size evidence required for a format-sensitive change. Do not
freeze a byte layout from intuition: first pin a checked-in miniature source
fixture, then compare candidate layouts using the same queries and exactness
corpus. Preserve source attribution and provenance in every produced bundle.

Every ticket follows one coordinator-authored, three-sub-agent chain. The
coordinator writes one self-contained `proposed` ticket from
[`planning/templates/ticket.md`](planning/templates/ticket.md), using the
shipped result of the previous ticket and the rolling frontier. It does not
prewrite a backlog. The independent stages are:

1. **Independent ticket review.** A read-only sub-agent reviews scope,
   assumptions, dependencies, acceptance criteria, failure cases, and fit with
   the frontier. The reviewer does not edit files. The coordinator records and
   resolves every material finding in the ticket, then returns the revision to
   the same reviewer. Only after that reviewer
   records approval may the coordinator mark the ticket `ready`, commit and
   push the reviewed ticket, and begin development.
2. **Independent development.** A fresh sub-agent receives the reviewed ticket
   and repository, marks it `in-progress`, implements only that scope, runs
   focused tests, records implementation evidence, and marks it `review`. It
   does not commit or push.
3. **Independent code review.** A third sub-agent, different from the ticket
   reviewer and developer, reviews the actual diff and tests read-only.
   It checks format safety, exactness, corrupt-input handling, unnecessary
   allocation, accidental full scans, source/license drift, performance proof,
   and scope creep. The developer resolves or explicitly rebuts every material
   finding with evidence and returns the diff to the same reviewer. Only after
   that reviewer records approval may final `make lint`, `make test`, and `make
   spec` gates run. Record the completed review evidence in the ticket, mark it
   `complete`, and commit and push the coherent implementation outcome.

For a ticket whose reviewed scope includes an irreversible or public external
effect, code review advances to `publication-ready`, not directly to
`complete`. The coordinator then commits and pushes the reviewed preparation,
requires that exact commit's remote gate to be green, and completes the pinned
hygiene audit before performing the named external effect. Only the
coordinator may perform it. Afterward, the coordinator records redacted audit
and effect evidence, marks the ticket `complete`, commits and pushes, and runs
the normal ticket cleanup. The narrow lifecycle is:

```text
review -> publication-ready -> commit/push -> green remote gate
       -> coordinator external effect -> complete -> commit/push -> cleanup
```

Production small outputs needed for review are generated by the coordinator
before code review. Developers and reviewers do not mutate external state.

The coordinator and all three sub-agents are separate roles. The coordinator
owns ticket authorship and remediation, orchestrates the chain, records
evidence, runs final gates, and commits and pushes independently approved work;
it does not implement product code or review its own ticket. Never ask an agent
to review its own implementation. Ticket findings return to the coordinator
and then the same ticket reviewer. Code findings return to the same developer
and then the same code reviewer. A material change to a reviewed-ready ticket
returns to the coordinator and ticket reviewer before development continues. Reviews happen
sequentially on the same intended diff; extra branches or worktrees are used
only for real concurrent work or isolation.

Developers do not commit or push. The coordinator is the only role that commits
and pushes, and only after the applicable reviewer approves.

Documentation is part of the implementation, not cleanup. Each ticket names
the durable and user-facing documents it changes. The developer updates them,
the code reviewer reviews them against behavior, and the coordinator checks
them for stale future/current claims before final gates. Any material final-gate
or stale-documentation finding returns to the same developer and then the same
code reviewer. If it exposes a scope defect, return the ticket to the
coordinator and same ticket reviewer before continuing development.

The reviewed-ready ticket is committed before development. The final
implementation commit includes the `complete` ticket with its implementation
and code-review evidence. Immediately afterward, remove that completed ticket
in a planning-cleanup commit and push it. This preserves the full audit trail in
git while returning `planning/tickets/` to active work only.
