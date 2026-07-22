# AGENTS.md — Pangopup

Pangopup is a GPL Rust workspace for a standalone Pangolin-compatible splice
service. The target product combines exact lookup of published GRCh38 SNV
scores with model fallback for lookup misses and supported non-SNVs. The
repository currently ships the source inspector, deterministic complete-corpus
builder and verifier, fixed 11-byte mmap reader, typed score-provider API, and
batch lookup CLI. Automatic asset installation, model inference/fallback, and
the HTTP service remain target work and are not implemented. Read `README.md`
first.

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

Every ticket follows this four-sub-agent chain, coordinated by an agent that
does not author, review, or implement the ticket:

1. **Independent ticket authorship.** A ticket-author sub-agent writes one
   self-contained `proposed` ticket from
   [`planning/templates/ticket.md`](planning/templates/ticket.md). It names the
   observable outcome, scope, hard decisions, dependencies, tests, performance
   proof, documentation changes, and exact gates.
2. **Independent ticket review.** A different read-only sub-agent reviews scope,
   assumptions, dependencies, acceptance criteria, failure cases, and fit with
   the frontier. The reviewer does not edit files. The coordinator records and
   routes every material finding to the same author for a change or evidence,
   then returns the revision to the same reviewer. Only after that reviewer
   records approval may the coordinator mark the ticket `ready`, commit and
   push the reviewed ticket, and begin development.
3. **Independent development.** A third sub-agent receives the reviewed ticket
   and repository, marks it `in-progress`, implements only that scope, runs
   focused tests, records implementation evidence, and marks it `review`. It
   does not commit or push.
4. **Independent code review.** A fourth sub-agent, different from the author,
   ticket reviewer, and developer, reviews the actual diff and tests read-only.
   It checks format safety, exactness, corrupt-input handling, unnecessary
   allocation, accidental full scans, source/license drift, performance proof,
   and scope creep. The developer resolves or explicitly rebuts every material
   finding with evidence and returns the diff to the same reviewer. Only after
   that reviewer records approval may final `make lint`, `make test`, and `make
   spec` gates run. Record the completed review evidence in the ticket, mark it
   `complete`, and commit and push the coherent implementation outcome.

The coordinator and all four sub-agents are separate roles. The coordinator
only orchestrates the chain, records mechanical evidence, runs final gates, and
commits and pushes independently approved work; it does not make substantive
ticket or product edits. Never ask an agent to review its own ticket or
implementation. Ticket findings return to the same author and then the same
ticket reviewer. Code findings return to the same developer and then the same
code reviewer. A material change to a reviewed-ready ticket returns to its
original author and reviewer before development continues. Reviews happen
sequentially on the same intended diff; extra branches or worktrees are used
only for real concurrent work or isolation.

Ticket authors also do not commit or push. The coordinator is the only role
that commits and pushes, and only after the applicable reviewer approves.

Documentation is part of the implementation, not cleanup. Each ticket names
the durable and user-facing documents it changes. The developer updates them,
the code reviewer reviews them against behavior, and the coordinator checks
them for stale future/current claims before final gates. Any material final-gate
or stale-documentation finding returns to the same developer and then the same
code reviewer. If it exposes a scope defect, return the ticket to the same
author and ticket reviewer before continuing development.

The reviewed-ready ticket is committed before development. The final
implementation commit includes the `complete` ticket with its implementation
and code-review evidence. Immediately afterward, remove that completed ticket
in a planning-cleanup commit and push it. This preserves the full audit trail in
git while returning `planning/tickets/` to active work only.
