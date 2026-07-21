# 004 — Typed mmap SNV lookup and CLI

Status: proposed

## Why

After Ticket 003 produces a certified complete bundle, Pangopup still needs the
first user-visible product slice: open that immutable bundle once, answer one or
many explicit GRCh38 SNVs, return every gene-specific published score exactly,
and quantify the real open plus 1/10/100 lookup costs.

This slice exposes the selected mmap reader through a narrow typed provider and
the CLI. It returns typed misses and source exceptions; it does not silently
fall through to an unimplemented model.

## Scope

- In `pangopup-core`, add the minimal stable lookup vocabulary:
  `GeneScoreRecord` (source Ensembl gene plus `PangolinScore`),
  `SourceReferenceAmbiguity` (gene, literal source reference `N`, published
  alternates, and omitted alternate), `LookupResult` (sorted `records`, sorted
  `source_reference_ambiguities`, and provenance identity), a typed lookup error,
  and one `ScoreProvider`
  capability accepting `Grch38Snv` plus an optional `EnsemblGeneId` filter.
  Results are small owned values sorted by gene; no mapped-byte lifetime or file
  layout enters the public API.
- In `pangopup-index`, implement the provider over one long-lived immutable mmap.
  Open performs only the cheap structural/identity validation established by
  Tickets 002/003. It does not hash or page through the payload. Lookup uses
  checked arithmetic, validates every touched rank/block/value, and performs no
  scan proportional to a chromosome, gene, or bundle.
- Default lookup returns all source-gene records for the genomic allele in
  deterministic Ensembl order. An optional gene filter narrows the result and
  never changes score semantics. No-gene and gene-filtered misses remain
  distinguishable only through request context, not invented annotations.
- A concrete query at a source `REF=N` locus adds a
  `SourceReferenceAmbiguity`; it never guesses the FASTA base or fabricates the
  missing alternate. A position may simultaneously return ordinary records and
  source ambiguities from overlapping genes. The optional gene filter applies
  to both collections.
- Extend `pangopup` with exact first-slice grammar:

  ```text
  pangopup lookup --bundle <PATH> \
    --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] \
    [--gene <ENSG>] [--format jsonl|table]
  ```

  Accept primary contig aliases `17`/`chr17` and the exact RefSeq accession
  aliases embedded by the bundle. Reject any assembly other than literal
  `GRCh38`; this is tuple parsing, not HGVS. Open the bundle once per invocation
  and reuse it for every repeated `--variant`.
- JSON Lines is the default stable machine output: one object per request with
  normalized variant, derived status, zero/one/many gene records, zero/one/many
  source ambiguities, and bundle/source provenance. Status is `found` when only
  records are nonempty, `ambiguous_source_reference` when only ambiguities are
  nonempty, `mixed` when both are nonempty, and `not_found` when both are empty.
  Score fields are fixed two-decimal JSON strings (`"0.35"`, `"-0.21"`, and
  normalized zero `"0.00"`); positions are JSON integers. The exact field set
  and examples below are contractual. `--format table` is a human adapter over
  the same typed result with exact columns `ASSEMBLY CONTIG POS REF ALT STATUS
  GENE GAIN_SCORE GAIN_POS LOSS_SCORE LOSS_POS SOURCE_REF PUBLISHED_ALTS
  OMITTED_ALT BUNDLE_ID`; alignment is not contractual. No TTY-dependent schema
  changes.
- Parse and validate the entire request batch before emitting output. Exit 0
  when all requests are syntactically valid and the bundle is usable, including
  `not_found`, `ambiguous_source_reference`, and `mixed` results. Exit 2 for
  CLI/input errors and 1 for bundle/open/decode failures. A batch never labels
  partial output as complete after a fatal bundle failure.
- Add `spec/snv-lookup.md` using the small deterministic bundle fixture/build
  helper from Tickets 002/003. Cover one hit, the TP53/WRAP53 overlapping hit,
  gene filtering, miss, `REF=N`, a synthetic mixed ordinary-plus-ambiguity
  position, 10/100 repeated distinct input handling,
  malformed variant, corrupt bundle, JSONL, and table output. Do not require the
  full bundle in ordinary gates.
- Add inside-out tests for open validation, all contig aliases, reference/ALT
  checks, boundaries, deterministic multiplicity, local corrupt payloads,
  concurrency (`Send + Sync` reader and simultaneous immutable lookups), and no
  payload-wide work during open.
- Add a release-mode benchmark over the full Ticket 003 bundle measuring:
  open-only; the actual release CLI in a fresh process for deterministic
  manifests containing exactly 1/10/100 requests that return exactly 1/10/100
  gene-score records; and a separate one-open library benchmark over those same
  result counts. Measure gene-filtered and all-overlap hits separately; random
  hits and misses; same-page and cross-contig queries; and output serialization
  separately. Reuse Ticket 002's selected-reader instrumentation and report warm
  p50/p95/p99, throughput, allocations, resident memory, logical encoded bytes
  decoded, unique mapped-file page numbers addressed by the query algorithm,
  and operating-system minor/major fault deltas separately. Never claim these
  measures are physical bytes read from storage.
- Use a documented defensible cold method: an artifact larger than available
  memory on isolated hardware, or an OS/device method that proves pages were not
  resident. If neither is available, report cold results as unmeasured rather
  than relabeling first-after-build queries as cold.
- Retain `planning/artifacts/004-snv-lookup-performance.md` with hardware,
  compiler/commit, bundle/query identities, commands, methodology, and results.
  This report must explicitly answer opening and returning 1, 10, and 100
  distinct splice scores.
- Update `README.md`, `architecture/design.md`, `architecture/index.md`,
  `planning/faq.md`, and `planning/frontier.md` to replace targets with measured
  claims only where supported.
- Excluded: automatic asset discovery/download, release publication, FASTA at
  runtime for lookup hits, model fallback, non-SNV support, model-result cache,
  HTTP, HGVS, transcript/protein projection, and clinical interpretation.

## Success Checklist

- The public provider and CLI return every matching source-gene record exactly;
  the overlapping `chr17:7686072 G>T` fixture returns distinct WRAP53 and TP53
  scores by default and the requested single record under each gene filter.
- Lookup hits reproduce exact centi-scores and relative positions from the
  checked fixture and a deterministic full-bundle oracle sample. No binary
  floating-point enters stored or comparison semantics.
- Empty (`not_found`), ambiguity-only, and mixed record-plus-ambiguity results,
  plus invalid variants, incompatible bundles, and touched-payload corruption,
  have distinct stable typed/CLI behavior.
- Tests prove cheap open does not full-hash or traverse the score payload, while
  explicit `pangopup-build verify` from Ticket 003 still detects whole-bundle
  corruption.
- One process opens one bundle once and safely serves concurrent lookups without
  an application cache or per-query file reopen.
- `spec/snv-lookup.md` covers the full observable matrix using only checked
  small fixtures.
- The retained performance report answers the requested 1/10/100 cases and
  separates process, open, lookup, page-fault, allocation, and rendering costs.
  Benchmarks are evidence, not threshold-based test assertions.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

### Return all overlapping source records by default

- Consideration: masking is gene-specific and one genomic allele can have
  several valid published records.
- Options: choose one gene; require a gene; return all records with an optional
  filter.
- Trade-offs: choosing is biologically false; requiring a gene burdens common
  callers and assumes outside annotation; returning all preserves the source
  and still permits the fast filtered path.
- Decision: all matches in Ensembl order are the default; `--gene` and the
  provider filter only narrow, never select implicitly.

### Use owned results over mmap-backed public borrows

- Consideration: borrowed packed bytes can avoid a tiny copy but expose reader
  lifetimes and private layout through every caller.
- Options: public zero-copy views; callback-only API; small owned typed records.
- Trade-offs: owned results may allocate for rare overlaps, but results are tiny,
  transport adapters are simpler, and format changes remain private.
- Decision: return small owned typed values. Benchmarks measure allocations; an
  internal small-result optimization is allowed only without changing API
  semantics.

### Open once and keep verification levels separate

- Consideration: one-shot CLI use includes open cost, while a service should not
  reopen or full-hash gigabytes per request.
- Options: reopen each query; global hidden singleton; explicit long-lived reader
  passed through one provider.
- Trade-offs: explicit ownership requires adapters to manage one object but is
  testable, deterministic, and service-ready.
- Decision: CLI opens once per invocation; future HTTP owns the same provider
  for process lifetime. Cheap open and explicit offline verify stay distinct.

### JSONL is stable; table is an explicit presentation

- Consideration: callers need machine stability and humans need inspectability,
  including batches with misses and multiple genes.
- Options: table only; JSON array; JSON Lines plus an explicit table format.
- Trade-offs: tables are fragile for machines; one array delays streaming;
  JSONL streams and composes, while an explicit table avoids TTY magic.
- Decision: JSONL default, one object per request; `--format table` is opt-in and
  derives from the same typed outcome.

The following five illustrative lines fix the JSONL schema (the all-zero bundle
hash stands for the real 64-hex identity). They show, in order, a gene-filtered
found-one result, the unfiltered found-many result at the same real fixture
allele, a miss, an ambiguity, and a synthetic mixed result. Record and ambiguity
arrays are always present and sorted by gene; provenance is present on every
outcome:

```json
{"assembly":"GRCh38","contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":[{"gene":"ENSG00000141499","gain_score":"0.35","gain_position":25,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:0000000000000000000000000000000000000000000000000000000000000000","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}
{"assembly":"GRCh38","contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":[{"gene":"ENSG00000141499","gain_score":"0.35","gain_position":25,"loss_score":"0.00","loss_position":-50},{"gene":"ENSG00000141510","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:0000000000000000000000000000000000000000000000000000000000000000","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}
{"assembly":"GRCh38","contig":"chr1","position":1,"ref":"A","alt":"C","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:0000000000000000000000000000000000000000000000000000000000000000","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}
{"assembly":"GRCh38","contig":"chr1","position":100,"ref":"A","alt":"C","status":"ambiguous_source_reference","records":[],"source_reference_ambiguities":[{"gene":"ENSG00000000003","source_ref":"N","published_alts":["A","C","G"],"omitted_alt":"T"}],"provenance":{"kind":"precomputed","bundle_id":"sha256:0000000000000000000000000000000000000000000000000000000000000000","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}
{"assembly":"GRCh38","contig":"chr1","position":100,"ref":"A","alt":"C","status":"mixed","records":[{"gene":"ENSG00000000005","gain_score":"0.00","gain_position":-50,"loss_score":"-0.10","loss_position":2}],"source_reference_ambiguities":[{"gene":"ENSG00000000003","source_ref":"N","published_alts":["A","C","G"],"omitted_alt":"T"}],"provenance":{"kind":"precomputed","bundle_id":"sha256:0000000000000000000000000000000000000000000000000000000000000000","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}
```

## Dependencies

- Ticket 003 complete, with one certified full bundle, deterministic small build
  fixture/helper, final selected format, and offline verifier on `main`.

## Notes

- This is a reviewed dependency-gated draft. Do not mark it `ready` or dispatch
  until Tickets 002 and 003 ship; then re-read current code/docs and obtain a
  fresh independent ticket review.
- A score count is gene-record count, not merely request count. Performance
  workloads must state both requests and returned records.
- Do not benchmark a CLI loop that reopens the bundle and present it as service
  lookup throughput. Report one-shot and long-lived cases separately.
- If build/spec helpers named here exist after Ticket 003, reuse them; otherwise
  define the minimum helper within this ticket.
- Evidence in this ticket is illustrative unless explicitly named as a retained
  artifact. Public files contain no machine paths or sibling-project references.
- Run exactly `make lint`, `make test`, and `make spec` from repository root.

## Independent Ticket Review

Reviewer: `next_ticket_set_review` (independent, read-only packet review)

Initial result: changes required. The coordinator accepted every finding: one
lookup result can represent ordinary records and source-reference ambiguities
together; the gene filter applies to both; JSONL/table wire contracts and all
four statuses are exact; and 1/10/100 benchmarks pin both request and returned
record counts while separating the real CLI from one-open library cost.
Page work now uses the same defensible logical/page/fault measures as Ticket
002.

Final packet re-review: approved with no remaining findings as a dependency-
gated `proposed` draft only. It must receive a fresh independent review against
the implementations produced by Tickets 002 and 003 before becoming `ready`.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending
