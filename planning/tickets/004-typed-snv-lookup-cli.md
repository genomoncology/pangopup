# 004 — Typed mmap SNV lookup and CLI

Status: complete

## Why

Ticket 003 shipped a reproducible certified-bundle builder and deleted its
generated full artifact. Pangopup still needs the first user-visible product
slice: rebuild and open that immutable bundle, answer one or many explicit
GRCh38 SNVs, return every gene-specific published score exactly, and quantify
the real open plus 1/10/100 lookup costs.

This slice exposes the selected mmap reader through a narrow typed provider and
the CLI. It returns typed misses and source exceptions; it does not silently
fall through to an unimplemented model.

## Scope

- In `pangopup-core`, add this exact object-safe capability:

  ```rust
  pub trait ScoreProvider: Send + Sync {
      fn lookup(
          &self,
          snv: Grch38Snv,
          gene: Option<EnsemblGeneId>,
      ) -> Result<LookupResult, LookupError>;
  }
  ```

  Add private-field public values, all deriving `Clone, Debug, Eq, PartialEq`,
  with public constructors and getters for every field:

  ```text
  GeneScoreRecord { gene: EnsemblGeneId, score: PangolinScore }
  SourceReferenceAmbiguity { gene: EnsemblGeneId,
    published_alternates: [DnaBase; 3], omitted_alternate: DnaBase }
  PrecomputedProvenance { bundle_id: String, source_doi: String,
    source_archive_md5: String, masked: bool, window: u32 }
  #[non_exhaustive] LookupProvenance::Precomputed(PrecomputedProvenance)
  LookupResult { records: Vec<GeneScoreRecord>,
    source_reference_ambiguities: Vec<SourceReferenceAmbiguity>,
    provenance: LookupProvenance }
  #[non_exhaustive] LookupError::CorruptProviderData
  ```

  `LookupError` also derives `Clone, Debug, Eq, PartialEq` and implements
  `Display` plus `std::error::Error`. Provenance stores `bundle_id` with its
  `sha256:` prefix and `source_archive_md5` as bare 32 lowercase hex.
  `SourceReferenceAmbiguity::source_reference()` returns literal `"N"`; the
  type cannot represent another source reference. Its constructor accepts gene
  plus omitted alternate and derives the canonical other-three array in
  `A,C,G,T` order; callers cannot construct the wrong length/order.
  `LookupResult::new` sorts both owned collections by gene, so its public
  constructor enforces the result invariant. No `IndexError`, offset, mmap
  lifetime, manifest type, `IndexReader`, or binary-layout detail enters this
  API. Convert or rename the current index-private `GeneScore`,
  `SourceAmbiguity`, and `LookupResult`; do not leave a second public result
  vocabulary.
- In `pangopup-index`, implement `ScoreProvider` on one long-lived opened bundle
  provider owning its immutable mmap and provenance. Bundle open has the actual
  shipped boundary: read and canonical-validate the capped (<=1 MiB) manifest;
  enumerate and `stat` the exact member set; mmap `scores.pgi`; scan/validate all
  segment, interval-tree, and exception metadata; and decode all exceptions.
  This is `O(segment_count + exception_count)` metadata work. It deliberately
  does not hash members or intentionally touch ordinary score payload. Tests
  distinguish algorithm-addressed pages from incidental OS readahead.
- Give bundle open a typed incompatibility classification rather than requiring
  adapters to inspect `IndexError` strings: schema/index-format version mismatch
  is `Incompatible` (or an equivalent typed variant), ordinary filesystem open
  failures remain I/O, and every other manifest/index structural problem is
  corrupt/invalid. CLI mapping is exact: open I/O -> `BUNDLE_IO`, typed
  schema/format mismatch -> `BUNDLE_INCOMPATIBLE`, other open corruption ->
  `BUNDLE_INVALID`, and post-open record decode failure -> `LOOKUP_CORRUPT`.
- Lookup uses checked arithmetic and performs no chromosome, gene, or bundle
  scan. Once an ordinary 11-byte record is addressed, validate its reserved
  bit, reference code, and all six gain/loss score-position pairs before
  selecting the requested alternate; corruption elsewhere in that addressed
  record is therefore `CorruptProviderData`. Untouched ordinary records remain
  lazy. Preserve the existing adversarial interval-tree proof. Complexity is:
  gene-filtered `O(log S + log E)` plus constant decode; unfiltered enumeration
  `O(log S + K)`; sorted public output
  `O(log S + K log K + log E + A log A)` with `O(K + A)` result allocation,
  where `S` is segments, `E` exceptions, `K` ordinary matches, and `A`
  ambiguities.
- Default lookup returns all source-gene records for the genomic allele in
  deterministic Ensembl order. An optional gene filter narrows the result and
  never changes score semantics. No-gene and gene-filtered misses remain
  distinguishable only through request context, not invented annotations.
- A concrete query at a source `REF=N` locus adds a
  `SourceReferenceAmbiguity`; it never guesses the FASTA base or fabricates the
  missing alternate. A position may simultaneously return ordinary records and
  source ambiguities from overlapping genes. The optional gene filter applies
  to both collections. Every syntactically valid concrete REF/ALT query at that
  exception coordinate returns the same matching-gene ambiguity, independent
  of the concrete pair requested, and never returns the exception's stored
  scores.
- Extend `pangopup` with exact first-slice grammar:

  ```text
  pangopup lookup --bundle <PATH> \
    --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] \
    [--gene <ENSG>] [--format jsonl|table]
  ```

  Accepted spellings are case-sensitive and exact: `1`..`22`, `X`, `Y`, `M`;
  `chr1`..`chr22`, `chrX`, `chrY`, `chrM`; and the exact 25 RefSeq accessions in
  the opened manifest. Reject zero-padded, lowercase, whitespace-padded, `MT`,
  and all other aliases. Assembly is literal `GRCh38`; POS is nonzero decimal
  fitting `u32` and must not exceed the opened manifest's contig length; REF/ALT
  are uppercase `A/C/G/T` and differ. This is tuple parsing, not HGVS. A valid
  concrete tuple whose REF does not match an ordinary indexed key is
  `not_found`, not a reference error: no runtime FASTA is present in this slice.
  Open once per invocation and reuse it for every repeated `--variant`.
  Tighten `Grch38Contig::from_str` itself to those primary contig spellings;
  bundle-aware RefSeq accession resolution remains in the CLI/provider adapter.
- JSON Lines is the default stable machine output: one object per request with
  normalized variant, derived status, zero/one/many gene records, zero/one/many
  source ambiguities, and bundle/source provenance. Status is `found` when only
  records are nonempty, `ambiguous_source_reference` when only ambiguities are
  nonempty, `mixed` when both are nonempty, and `not_found` when both are empty.
  Score fields are fixed two-decimal JSON strings (`"0.35"`, `"-0.21"`, and
  normalized zero `"0.00"`); positions are JSON integers. Output is compact
  UTF-8, one newline-terminated object per request in input order, with exact
  fields and key order as illustrated below. `source_archive_md5` is the bare
  32 lowercase hex characters after stripping manifest `md5:`; `bundle_id`
  retains `sha256:`.
- `--format table` is exact tab-separated UTF-8 with one header per invocation:
  `ASSEMBLY CONTIG POS REF ALT STATUS GENE GAIN_SCORE GAIN_POS LOSS_SCORE
  LOSS_POS SOURCE_REF PUBLISHED_ALTS OMITTED_ALT BUNDLE_ID`. Requests stay in
  input order. For each request, emit ordinary record rows first in gene order,
  then ambiguity rows in gene order; emit one row per item and one placeholder
  row for `not_found`. Repeat status and request/bundle fields on every row; use
  `.` for every inapplicable cell; encode `PUBLISHED_ALTS` as comma-separated
  canonical bases such as `A,C,G`. The header and every row, including the
  final row, are LF-terminated. No alignment or TTY-dependent behavior.
- Batch behavior is transactional before stdout: parse the entire batch; open
  once; apply manifest-length validation; perform every lookup; serialize the
  complete JSONL or table response into memory; only then start one stdout
  write. Lookup/decode/serialization failure therefore writes zero stdout. An
  operating-system failure during that final write can inherently leave a
  partial byte stream and returns `OUTPUT_IO`.
- Lookup-subcommand failure writes no stdout and exactly one compact JSON line
  to stderr:
  `{"status":"error","code":string,"message":string,"details":null}`.
  Codes are closed: `CLI_USAGE`, `INVALID_VARIANT`, `INVALID_GENE` exit 2;
  `BUNDLE_IO`, `BUNDLE_INCOMPATIBLE`, `BUNDLE_INVALID`, `LOOKUP_CORRUPT`, and
  `OUTPUT_IO` exit 1. Code/details are contractual; human message wording is
  not. Exit 0 includes found, miss, ambiguity, and mixed results. Preserve
  ordinary help/version behavior.
- Add a dedicated checked lookup source/reference fixture built through Ticket
  003's production bundle path. It contains the real `chr17:7686072 G>T`
  WRAP53/TP53 source rows; an ambiguity-only locus; a mixed ordinary/`REF=N`
  locus; a proved miss; and at least 100 distinct ordinary hits. Its compact
  deterministic gzip FASTA contains all 25 required accession records and is
  long enough at used coordinates. `spec/snv-lookup.md` builds its own bundle
  and never relies on another spec's execution order. Cover one/many hits, both
  gene filters, miss, ambiguity, mixed, 10/100 distinct input requests,
  malformed inputs, every output status, exact JSONL/table bytes, and each
  corruption layer without requiring external data.
- Add inside-out tests for the exact alias grammar and length bounds;
  deterministic multiplicity; an addressed record whose unrequested pair is
  corrupt; an untouched corrupt ordinary record remaining lazy; and the actual
  validation layers: manifest/header/directory/tree/exception corruption fails
  open, same-size NOTICE substitution and untouched payload corruption may pass
  open, addressed corruption fails lookup, and `pangopup-build verify` catches
  member hashes and all payload corruption. Add a compile-time `Send + Sync`
  assertion and barrier-started concurrent lookups through one `Arc` provider,
  with every result compared with a serial oracle.
- The complete Ticket 003 bundle no longer exists. Before full-oracle and
  performance work, build it anew from explicit operator-supplied
  `PANGOPUP_SOURCE_DIR` and `PANGOPUP_GRCH38_FASTA` using the current release
  binary, then independently run `pangopup-build verify`. Require the known
  corpus counts, logical digest, reference/source identities, and exact Ticket
  003 `scores.pgi` SHA-256
  `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`.
  This ticket does not authorize a format/writer change; if that hash cannot be
  reproduced, stop and open a new format ADR/ticket. Record the new
  manifest/bundle identity, which changes because its
  builder source digest includes Ticket 004 core/index changes. Delete the full
  bundle after all Ticket 004 evidence is retained.
- Retain `planning/artifacts/004-query-manifest.tsv`: request ID, workload
  class, stable order, variant, optional gene, expected request count, and
  expected returned gene-record count. Retain
  `planning/artifacts/004-full-oracle.jsonl`: provenance-free expected records
  and ambiguities for every retained request, independently extracted from the
  source TSVs rather than decoded from `scores.pgi`; independently establish
  misses by complete relevant-source inspection. Bind both files in the report
  to source-member identity, reference identity, `scores.pgi` hash, and bundle
  ID.
- Add a release benchmark harness with four modes: fresh in-process open-only;
  the real release CLI as one fresh child per complete 1/10/100 batch (parse,
  open, lookup, render, write); one-open library lookup only; and
  serialization-only over materialized results into a memory buffer. Primary
  filtered and unfiltered manifests contain exactly 1/10/100 requests returning
  exactly 1/10/100 gene records. Every filtered batch uses one batch-global gene
  shared by all its variants and is one child invocation with one `--gene` plus
  repeated `--variant`; never split it into per-query children. Add separate
  true-overlap stress, deterministic
  seeded random hits/misses, same-page, and cross-contig workloads. Record both
  request and result counts.
- Benchmark warmup is 20 unretained samples followed by 100 retained samples;
  p50/p95/p99 use nearest-rank over sorted retained latencies. Report batches/s
  and records/s, deterministic seed/universe, a fixed 4 KiB logical mapped-page
  number, and CLI stdout sink. Fresh CLI reports wall latency, throughput, child
  minor/major faults, peak RSS, and output bytes. In-process open/library/
  serialization report latency, allocation calls/bytes, fault deltas, and RSS
  delta; serialization-only reports JSONL and table as separate workloads.
  Lookup-only additionally reports logical bytes decoded and unique
  algorithm-addressed mapped pages. Use `N/A` where a metric is unavailable;
  do not claim child allocation counts without a separately disclosed
  instrumented build. Never call logical/page metrics physical storage reads.
- Use a documented defensible cold method: an artifact larger than available
  memory on isolated hardware, or an OS/device method that proves pages were not
  resident. If neither is available, report cold results as unmeasured rather
  than relabeling first-after-build queries as cold.
- Retain `planning/artifacts/004-snv-lookup-performance.md` with hardware,
  compiler/commit, regenerated bundle/query/oracle identities, exact commands,
  methodology, per-mode applicability, and results.
  This report must explicitly answer opening and returning 1, 10, and 100
  distinct splice scores.
- Update `README.md`, `architecture/design.md`, `architecture/index.md`,
  `architecture/decisions/0006-index-format-selection.md`, `spec/cli.md`,
  `planning/faq.md`, and `planning/frontier.md`. Resolve the old runtime
  reference-mismatch sentence to the no-FASTA miss semantics above; replace
  unimplemented/remaining-choice language; and distinguish measured library
  lookup, fresh CLI, warm state, and unmeasured cold behavior. No HTTP or model
  latency claim follows from this ticket.
- Excluded: automatic asset discovery/download, release publication, FASTA at
  runtime for lookup hits, model fallback, non-SNV support, model-result cache,
  HTTP, HGVS, transcript/protein projection, and clinical interpretation.

## Success Checklist

- `ScoreProvider: Send + Sync` and every owned core result/provenance/error type
  match the exact API contract; no index-private duplicate becomes a competing
  public vocabulary.
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
- Tests pin the metadata-scan open boundary, all-pairs addressed-record
  validation, untouched-record laziness, exact aliases/ranges, transactional
  batch stdout, closed JSON errors, and byte-exact JSONL/table multiplicity.
- One process opens one bundle once and safely serves concurrent lookups without
  an application cache or per-query file reopen.
- `spec/snv-lookup.md` covers the full observable matrix using only checked
  small fixtures.
- The retained performance report answers the requested 1/10/100 cases and
  separates process, open, lookup, page-fault, allocation, and rendering costs.
  Benchmarks are evidence, not threshold-based test assertions.
- A current-commit full rebuild and independent verify reproduce the pinned
  corpus/logical/index identity. The retained source-derived oracle and query
  manifest are bound to that bundle, and the generated bundle is deleted.
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

- Ticket 003 shipped in commits `a1b9a38`/`338b401`, with the deterministic
  production builder, fixed-v1 format, offline verifier, small production build
  fixture, and retained full-corpus evidence on `main`. No full bundle is
  retained; this ticket rebuilds one explicitly for its non-gate oracle and
  benchmark work.

## Notes

- Tickets 002 and 003 are shipped. Do not dispatch until the fresh dependency-
  time review below approves the revised contract.
- A score count is gene-record count, not merely request count. Performance
  workloads must state both requests and returned records.
- Do not benchmark a CLI loop that reopens the bundle and present it as service
  lookup throughput. Report one-shot and long-lived cases separately.
- Reuse Ticket 003's production build helper but add the dedicated lookup
  fixture specified above; do not pretend Ticket 002's bare `.pgi` or Ticket
  003's two-gene fixture contains the required lookup matrix.
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

Fresh dependency-time reviewer: `ticket_004_review` (independent, read-only).

Initial result: changes required. The coordinator accepted every finding and
revised the ticket to require a current-commit full rebuild, a source-derived
retained oracle, and a dedicated production-path lookup fixture; to pin the
typed Rust API, exact alias/range/REF=N behavior, byte-exact JSONL/table and
closed error contracts, and transactional batch output; to describe the actual
metadata-scan open boundary, addressed-record validation, sorting complexity,
and corruption layers; and to separate benchmark modes, observable metrics,
samples, workloads, and documentation updates.

Final dependency-time re-review: approved with no remaining findings. The
reviewer confirmed the extensible provider/provenance API, typed open/decode
classification, invariant-preserving ambiguity/results, production-path
fixture, transactional wire contracts, unconditional fixed-v1 identity gate,
source-derived oracle lifecycle, and four-mode benchmark are internally
consistent and implementable against shipped Tickets 001–003.

## Implementation Evidence

Developer: `ticket_004_development`

- Added the invariant-preserving owned lookup vocabulary and object-safe
  `ScoreProvider: Send + Sync` contract to `pangopup-core`, including exact
  contig parsing. `pangopup-index::BundleOpen` now implements that contract,
  owns one long-lived mmap plus provenance, classifies incompatibility
  separately, validates all pairs in an addressed ordinary record, and keeps
  untouched ordinary payload lazy.
- Added the transactional `pangopup lookup` CLI with exact GRCh38 tuple and
  manifest-accession grammar, length and gene validation, one open per batch,
  all four statuses, byte-stable JSONL/table output, and the closed JSON error
  and exit-code contract. Added a production-path fixture and
  `spec/snv-lookup.md` for found-one/many, both gene filters, miss, ambiguity,
  mixed, 10/100 batches, exact serialization, typed failures, corruption
  layering, and zero-stdout transactional failure.
- Code-review remediation put the shipped CLI and benchmark behind one shared
  renderer and made every one of the benchmark's 1,560 warmup/retained child
  outputs pass an exact-byte comparison. It also made opened-bundle manifest,
  identity, index, and frozen provenance state private with read-only access;
  rejects invalid contig syntax before bundle I/O; bounds manifest reads before
  allocation; and preserves normal top-level and lookup help/version behavior.
  Final re-review also moved all timing-vector allocation and initialization
  before the in-process allocation/fault/RSS baselines and added an
  empty-operation runtime guard proving the sampler itself reports zero
  allocations. The full harness was rerun: every lookup/serialization row
  dropped the exact contaminating 0.01 calls and 16 bytes per batch, with all
  latency and resource observations replaced from the same consistent run.
- Added inside-out unit/integration coverage for alias grammar, constructor
  invariants, deterministic multiplicity, unrequested-pair corruption,
  untouched-record laziness, open/lookup/verify corruption boundaries, and a
  barrier-started concurrent `Arc` provider compared with a serial oracle.
  The final observable matrix exhaustively covers primary and RefSeq aliases,
  bounds and rejected spellings, all four byte-exact statuses in both formats,
  multiplicity/final-LF behavior, all 12 concrete pairs at both `REF=N`
  fixtures with filtering, and manifest/directory/tree/exception corruption,
  including an oversized sparse manifest rejected before content allocation.
  Updated the requested README, architecture, ADR, CLI spec, FAQ, and frontier
  documentation to the shipped lookup and no-runtime-FASTA miss semantics.
- Fresh current-source production build and independent verify accepted bundle
  `sha256:3a5d7de6aacf2aada1ff327764e21d5142ad8a534f7f861fb127576a664d5ee2`.
  It reproduced the pinned 15,033,158,255-byte `scores.pgi` hash
  `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27`,
  all known corpus counts, and identical 4,099,255,665-record source/decoded
  digest `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31`.
  The regenerated full bundle and rejected earlier copies were deleted after
  evidence capture.
- Retained `planning/artifacts/004-query-manifest.tsv`
  (`sha256:36644941adbf78419ff9cf5c42ae57e46cca336b1099f9c9e9902d0b30ea8cfa`),
  `004-full-oracle.jsonl`
  (`sha256:c93c75bbf61b39f7fd88c868b2fe01eb29117e24c6c09df565275cc709a05119`),
  and the full-source extractor. A complete new scan of all 19,913 gzip members
  reproduced the oracle byte-for-byte; 260 requests in 13 one-open CLI groups
  matched it exactly. After code-review source changes, source/query/index
  identities remained unchanged, so the retained oracle was not needlessly
  regenerated; the 260-request comparison was rerun against the final bundle
  and again passed. Both seeded hit and independently proved miss vectors use
  the documented `0x50414e47` LCG sample.
- Retained `planning/artifacts/004-snv-lookup-performance.md`. Warm p50 open was
  1,167.643 us. Fresh release CLI p50 for filtered 1/10/100 records was
  2,566.029 / 2,566.490 / 2,897.891 us; one-open lookup-only p50 was
  0.441 / 5.521 / 44.628 us. The report retains all p50/p95/p99, throughput,
  allocation, fault, RSS, logical-byte/page, serialization, stress, hardware,
  compiler, identity, command, and applicability evidence. Cold remains
  explicitly unmeasured because this host could not prove nonresidency.
- Deviation disclosed in the report: an initial full run accidentally used the
  stale pre-ticket release builder. Although it reproduced the pinned index
  hash, its builder digest/bundle ID were rejected. The builder was rebuilt
  from current source and the complete full build, verify, oracle, and
  benchmark workflow was repeated against the accepted identity above; no
  format change was made.
- Final gate: `make lint`, `make test`, and `make spec` pass; mustmatch reports
  61 passing specifications.

## Adversarial Code Review

Reviewer: `ticket_004_code_review` (independent, read-only)

Initial result: changes required. The reviewer found six issues despite a green
gate: the serialization benchmark duplicated rather than exercised the shipped
renderer; opened provider provenance remained publicly mutable; invalid contig
syntax could be masked by bundle I/O; the manifest cap was checked after an
unbounded read; the exact output/alias/ambiguity/corruption matrix was
incomplete; and lookup help was not ordinary CLI help.

First re-review: five findings resolved, one additional evidence defect found.
The benchmark allocated its 100-sample timing vector after resetting allocation
and resource counters, contaminating every in-process row by 0.01 calls and 16
bytes per batch and potentially contaminating fault/RSS observations.

Final result: approved with no remaining findings. The reviewer verified the
shared production renderer and 1,560 byte comparisons, private frozen provider
state, pre-I/O syntax validation, bounded manifest reads, exhaustive observable
and corruption coverage, ordinary help/version behavior, current-source full
recertification, independently reproduced oracle identities, corrected
operation-only benchmark accounting with a zero-allocation harness guard,
artifact deletion, and the final 61-spec gate.
