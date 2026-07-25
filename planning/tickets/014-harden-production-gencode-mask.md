# 014 — Harden the production GENCODE mask bundle and provider

Status: ready
Contract identity (SHA-256 of this ticket with this value set to `pending`):
`ca7341aa36c6b0f55821d3a315f6d9ac2accdb77bd0ad93baebb662b72176a28`
Base revision: `9fd83023e7be7b0f0f4fd0b0c518bee0b5d45c1f`

## Why

Ticket 012 authenticated Pangolin's exact GENCODE v38 masking semantics and
selected constant-membership domains by a retained complete-source benchmark.
Its `PGMBEN01` files are deliberately benchmark-only. They are not a runtime
format, installable bundle, or supported provider.

Ticket 013 removed unrelated SNV/reference provenance coupling. The next
independent runtime input required by model inference is therefore a compact
production mask: one immutable bundle that can answer which exact, ordered
GENCODE genes and exon boundaries apply at a GRCh38 point.

This ticket hardens only that selected representation. It does not execute
Pangolin, rerun the Ticket 012 capture/comparison, add public installation or
network behavior, or route a variant to model inference.

## Pinned facts

- The selected logical profile is
  `pangolin-1.0.2-5cf94b8-grch38-v1`.
- The retained authenticated source is read-only:
  `/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/`.
- Its only production-build inputs are:
  - `canonical.jsonl`: 29,322,409 bytes,
    SHA-256
    `a23a24b9b421cc6790111cfb852ea375169d98cbeec0bcf4469194f412ee3014`;
  - `inventory.json`: 647 bytes,
    SHA-256
    `cb4889d00d1addb3a09a1738c5da10561cba69b267e041d6bbe9488c80e0713b`,
    the closed Ticket 012 inventory that binds the same logical-stream identity
    and exact counts below.
- The source inventory contains 60,649 exact versioned genes, 60,605 stable
  IDs, 44 `_PAR_Y` genes/stable-ID collisions, 591,404 normalized exon
  boundaries, 88,202 constant-membership domains, and exactly 25 primary
  contigs. It contains 30,769 plus-strand and 29,880 minus-strand genes and no
  duplicate exact ID or boundary-empty gene.
- The maximum boundary count for one gene is 726. Every exact ID is at most 64
  bytes. The builder must derive and enforce the maximum plus/minus cardinality
  of a domain before using a 16-bit count.
- The canonical stream retains inclusive GTF spans, while membership is
  Pangolin's effective `(start,end]`. Query results are plus strand followed by
  minus strand; within each strand they retain the authenticated upstream
  `rank`. A stable-ID filter is applied only after contig selection and never
  merges chrX with its `_PAR_Y` chrY copy.
- GENCODE release 38 source identities remain:
  - GTF gzip: 46,556,621 bytes,
    `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050`;
  - gffutils database: 380,366,848 bytes,
    `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6`;
  - final Ticket 012 capture contract:
    `ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb`.
  Those large inputs are provenance, not inputs to this build.
- The selected benchmark member is 6,703,320 bytes and compressed to
  3,933,486 bytes with the pinned Ticket 012 Zstandard recipe. Its retained
  p50/p95 was 171/331 ns with zero warmed allocation and median/p95 payload
  pages 7/9. It is evidence only and must not be copied, reheadered, or opened
  by the production builder.
- The fixed performance workload is
  `planning/artifacts/012-performance-manifest.jsonl`: 163,320 bytes,
  SHA-256
  `ffcf61425a69546c79405c8bbfe01cda77c86051d6828a05a74fe4b45e6c1473`.
- The retained compatibility corpus is 227,060 bytes with domain-separated
  aggregate identity
  `c077d400230fc7df83242d2737a850b2709299be990f521599b0e55735ff55e3`.
  Its canonical `manifest.json` is 5,337 bytes with SHA-256
  `fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b`.
  Its `cases.jsonl` is 220,071 bytes with SHA-256
  `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`.
- Normal tests use only `tests/fixtures/gencode-mask-mini/` and the bounded
  compatibility corpus. They never open the retained production stage.

## Scope

### Included — production format and typed API

- Add `pangopup-index::mask` as a production-only module, separate from
  `mask_candidates`.
- Define a closed little-endian `PGMASK01` v1 member with format identity
  `pangopup.mask.domains.v1`. It contains:
  1. a fixed header and five checked sections;
  2. exactly 25 canonical-contig directory entries;
  3. fixed gene records retaining contig, strand, exact versioned/PAR identity,
     inclusive source span, rank, and boundary range;
  4. normalized `u32` exon boundaries;
  5. one 16-byte domain record:
     `begin:u32, end:u32, posting_start:u32, plus_count:u16,
     minus_count:u16`;
  6. `u32` gene postings already ordered as plus-by-rank then minus-by-rank.
- Keep the established 160-byte header, 40-byte contig-directory record,
  40-byte gene record, 4-byte boundary/posting records, and 16-byte domain
  record. Every offset, count, stride, alignment, reserved byte, and padding
  byte is canonical and checked. The production magic/encoding discriminator
  must reject every `PGMBEN01` candidate.
- The production posting layout removes the candidate reader's scratch-match
  vector, per-query sort, and second gene decode. It must not increase the
  complete member beyond 6,703,320 bytes.
- Expose typed `MaskStrand`, `MaskQueryGene`, reusable caller-owned
  `MaskQueryBuffer`, immutable `MaskProvenance`, and a `MaskProvider:
  Send + Sync` trait in `pangopup-index::mask`. Do not add mask types to
  `pangopup-core`: changing that shared module would churn unrelated
  SNV/reference builder identities.
- `MaskProvider` accepts typed `Grch38Contig` and `GenomicPosition`, an optional
  stable `EnsemblGeneId` filter, and caller-owned output. A successful miss
  leaves empty plus/minus slices. Every error clears the buffer.
- The concrete `MaskBundleOpen` owns held read-only descriptors and one
  read-only mmap. A query returns exact plus/minus records and caller-buffer
  boundary slices without allocation when capacity is sufficient.

### Included — immutable three-member bundle

- Define closed canonical manifest schema `pangopup.mask.bundle.v1`, production
  profile `pangolin-1.0.2-5cf94b8-grch38-v1`, and independent synthetic profile
  `pangopup-mask-mini-v1`.
- A bundle contains exactly:
  - `NOTICE`;
  - `manifest.json`;
  - `mask.pgm`.
- The manifest binds its schema/profile/format, package version, independent
  mask-builder source fingerprint, exact GENCODE release/GTF/database/capture
  identities, canonical logical-stream identity, all logical counts, exact
  member sizes/SHA-256/media types, attribution links, and a statement that
  Pangopup transformed the source annotation.
- Its member entries cover `NOTICE` and `mask.pgm`. `manifest.json` cannot
  contain its own digest: the exact canonical manifest bytes are instead the
  bundle identity.
- The exact canonical manifest bytes are the bundle identity. Unknown fields,
  noncanonical JSON, unsupported profiles/formats, unqualified counts, or
  inconsistent member metadata are rejected.
- The production `NOTICE` names GENCODE release 38, the exact GTF URL, release,
  data-access, and citation-guidance links, and describes the transformation.
  It must not label GENCODE as the separately CC BY 4.0 Zenodo SNV-score
  dataset. The miniature notice states clearly that it is synthetic GPL-only
  test data.
- Bundle open uses a held no-follow directory and no-follow member descriptors,
  requires the exact member set, regular-file type, link count one, manifest
  size at most 64 KiB, notice size at most 16 KiB, and mask member size at most
  8 MiB. It never path-reopens a validated member.
- Cheap open validates canonical manifest/notice metadata, the complete fixed
  header and 25-entry directory, section arithmetic/order/alignment, and member
  extent. It does not hash or scan genes, boundaries, domains, or postings.
  Touched records are range- and canonical-validated during query; touched
  corruption is a typed error and never a panic or partial result.

### Included — deterministic builder and certification

- Add production construction/certification in a new bounded
  `pangopup-build` module. Do not modify the Ticket 012 `mask.rs`,
  `mask_candidates.rs`, candidate binary, or `build.rs`.
- Parse `canonical.jsonl` as bounded, closed JSON Lines; authenticate its exact
  size/SHA-256 before publication; authenticate and cross-check the inventory;
  reject malformed schema/order/rank/contig/identity/span/boundary/count data;
  and independently derive domains and strand-partitioned postings.
- Open canonical source, inventory, compatibility corpus, and performance
  workload through held no-follow descriptors with regular-file/link-count
  checks. Hash and parse from those same handles after rewinding; never
  authenticate one path instance and consume another. The qualification report
  records all four exact identities.
- Construction is deterministic, cancellation-aware, bounded to at most
  128 MiB peak Rust heap on the complete source, writes private no-follow
  staging files, syncs them, and publishes no-replace only after complete
  certification. A failed or interrupted build never leaves a runtime bundle.
- Certification hashes every member, independently decodes the complete
  production member, reconstructs the canonical logical gene stream and
  domain edges, replays all retained compatibility mask cases, and compares
  the production provider with the direct canonical oracle at every domain
  witness and every `start`, `start+1`, `end`, and `end+1` edge.
- Add the independent builder domain
  `pangopup.mask-builder-source.v1` using Ticket 013's length-framed
  compile-time evidence model. Its inventory covers only the production
  format/provider, builder/certifier, bounded shared support, compatibility
  oracle, root wiring, and exact isolated locked Linux dependency
  closure/features. Source/manifest dependency derivation, alias handling,
  independent digest recomputation, and causal/unrelated mutation controls
  must be as discriminating as the established SNV/reference proof.
- The exact Ticket 012 candidate fingerprint
  `fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816`
  and Ticket 013 fingerprints must remain unchanged:
  - SNV:
    `85126cbb4bbc008a475b0b941447fb7a24f299abb1754a1c10582912a522eb2d`;
  - reference:
    `252f60fd8ea809fa0a3b583bf3a7ddb99601fef67b21a227264e8fa55b873e24`.
- Keep the initial production lifecycle private behind the existing
  `mask-qualification` feature. It may add a feature-gated branch to
  `pangopup-build`, but no command appears in the ordinary binary/help surface
  and no new Cargo dependency is needed. This avoids widening the unresolved
  public maintainer-help contract.

### Included — one retained complete build and qualification

- Before production access, the developer implements and exercises only
  miniature tests. The independent code reviewer reviews the complete code,
  test, documentation, and exact proposed production invocation first.
- After that approval, the coordinator may authorize the same developer to run
  exactly one complete build/qualification from the two pinned retained inputs
  into:
  `/home/ian/workspace/data/pangopup-mask-production-014/`.
  The command must preflight the exact input identities and refuse replacement.
- Build and benchmark the held staging bundle. Publish it no-replace to the
  runtime production path only after every semantic, corruption, latency,
  allocation, page, heap, open-read, size, identity, and report check passes.
  A failed stage is moved under a clearly marked non-runtime quarantine path
  with a bounded failure report and no three-member bundle at its root.
- The retained qualification uses Ticket 012's checked 1,000-query manifest,
  six rounds of 10,000 warmups plus 100,000 measured queries on CPU 0, and the
  production provider with a preallocated output buffer. The optimized release
  executable is hashed before and after measurement. The report records its
  identity, Rust compiler, target, build profile, selected/allowed CPUs, CPU
  model, kernel, governor, power state, and logical page size.
- Measurement reproduces Ticket 012's exact nearest-rank arithmetic:
  - each round executes the ordered manifest as
    `queries[operation % 1_000]`: exactly 10 complete warmup cycles followed
    by exactly 100 complete measured cycles, with no filtering, grouping,
    shuffling, reordering, or substitution;
  - sort each round's 100,000 individual query durations and take p50 at index
    49,999 and p95 at index 94,999;
  - sort the six round p50 values and separately the six p95 values, taking
    headline index 2 for each;
  - trace each of the 1,000 workload queries once outside timing, sort page
    counts, and take median index 499 and p95 index 949.
- Semantic replay, capacity discovery, page tracing, and warmup occur before
  the timed/allocation window. Each latency sample starts immediately before
  the provider query and ends immediately after it; checksum/black-box work is
  outside the sample. Allocation counters are reset after warmup and cover the
  complete 100,000-query timed loop. Open-heap tracking starts from a captured
  live-allocation baseline immediately before `MaskBundleOpen` construction
  and reports the maximum increase until construction returns. The open-read
  counter covers only bytes the constructor deliberately reads from
  `mask.pgm`, not manifest/notice reads or later queries. Builder-heap tracking
  starts immediately after process/argument setup and before opening,
  authenticating, or preflighting any input. It includes input authentication,
  canonical/inventory parsing, layout construction, file construction,
  complete semantic certification, and assembly of every qualification-report
  value. The peak is frozen after those build/certification values exist and
  before serializing the report; report serialization is deliberately excluded
  to avoid a self-referential measurement and is separately bounded at 64 KiB.
- The 214/414 ns limits are the rounded-up 125-percent ceilings over Ticket
  012's selected 171/331 ns (`ceil(171×1.25)`, `ceil(331×1.25)`). The run must
  establish:
  - exact complete logical equivalence and compatibility-case replay;
  - p50 at most 214 ns and p95 at most 414 ns;
  - zero warmed allocation calls and bytes;
  - median/p95 logical payload pages at most 7/9;
  - open peak Rust heap at most 256 KiB;
  - at most 4,096 mask-member bytes deliberately read during cheap open;
  - member size at most 6,703,320 bytes;
  - builder peak Rust heap at most 128 MiB.
- Record, but do not gate on, pinned level-9 Zstandard size, open latency,
  process RSS, and page faults. This ticket does not select another codec.
- Preserve the successful qualified bundle and bounded qualification report.
  Commit only
  a small deterministic JSON report and prose evidence, not the runtime member
  or private source. There is no routine full-source verifier or unchanged
  rerun. A threshold or correctness failure is preserved and returned to
  design review rather than silently retried.
- The same adversarial code reviewer then checks the retained identities,
  report, repository diff, and absence of unreviewed code changes before final
  gates.

### Included — tests and documentation

- Extend the independent 25-contig miniature to prove exact build bytes,
  profile separation, all membership edges, plus/minus/rank order, stable
  filtering, PAR separation, empty-boundary genes, maximum accepted local
  cardinalities, reusable-buffer behavior, deterministic page traces, zero
  warmed allocation, and `Send + Sync` concurrent reads.
- Add malformed/corruption controls for source JSONL, manifest/notice/member
  set and types, magic/version/encoding, header/section arithmetic,
  contig-directory ordering, domain ordering/ranges/count splits, postings,
  gene identities/spans/ranks, boundary order/ranges, truncation, appended
  bytes, reserved bytes, symlinks, hard links, and post-open path replacement.
- Prove cheap open accepts an untouched payload corruption, an unrelated query
  still succeeds, a query that touches it returns typed corruption, and the
  caller buffer is empty afterward.
- Prove the private parser/lifecycle branch is absent from a normal
  `pangopup-build` build. No public CLI/spec behavior changes; the existing
  outside-in specifications remain the regression boundary.
- Add:
  - `architecture/decisions/0013-production-gencode-mask-bundle.md`;
  - `planning/artifacts/014-production-gencode-mask.md`;
  - a bounded machine-readable qualification report under
    `planning/artifacts/`.
- Update every current-state/future-state mask claim in:
  - `README.md`;
  - `architecture/README.md`;
  - `architecture/design.md`;
  - `architecture/index.md`;
  - `architecture/runtime-data.md`;
  - `planning/frontier.md`.

### Excluded

- Reading the retained `PGMBEN01` candidates or rerunning Ticket 012 capture,
  promotion, preparation, exhaustive comparison, or benchmark.
- Reading the production GTF, SQLite database, Python environment, model
  checkpoints, RefSeq FASTA, SNV source archive, or production SNV/reference
  bundles.
- Editing any current Ticket 012 fingerprint input, rebuilding an existing
  asset, or changing SNV/reference member/manifest formats or identities.
- Public mask build/help/version commands, transport compression, release
  assets, download/sync, XDG installation, active-profile selection,
  repair/GC/rollback, or publication.
- Model/checkpoint packaging, tensor-runtime selection, inference, variant
  normalization/routing, result caching, HTTP, Docker, systemd, or deployment.
- A second mask codec, interval/range API, general gene annotation API, HGVS,
  transcript/protein projection, consequences, disease knowledge, GRCh37, or
  liftover.

## Success Checklist

- [ ] `PGMASK01` is a distinct, deterministic, closed production format; no
      `PGMBEN01` member is accepted or copied.
- [ ] One exact three-member miniature bundle builds twice byte-identically,
      opens through held no-follow descriptors, and returns the fixture oracle
      through the typed `MaskProvider`.
- [ ] Query semantics preserve `(start,end]`, exact versioned/PAR identity,
      plus-then-minus upstream order, normalized boundaries, post-contig stable
      filtering, empty misses, and buffer clearing on failure.
- [ ] Cheap open remains bounded and payload-lazy; touched-corruption,
      file-substitution, member-set/type/link, canonical-manifest, malformed
      source, arithmetic, and resource controls discriminate the intended
      failures.
- [ ] Preallocated warmed queries allocate zero bytes; concurrency and logical
      page-trace controls pass.
- [ ] The artifact-specific mask provenance proof is independently
      recomputable and isolated; Ticket 012 and Ticket 013 exact identities
      remain unchanged.
- [ ] The single retained full build passes complete semantic certification
      and every fixed latency/allocation/page/heap/size threshold without
      reading or rebuilding another production input.
- [ ] The retained bundle/report are preserved, bounded evidence is committed,
      and no routine long verifier or public maintenance command is added.
- [ ] README, architecture, ADR, frontier, and artifact evidence distinguish
      shipped production bundle/provider from future transport/install/model
      integration.
- [ ] `make lint`, `make test`, and `make spec` pass.

Focused commands may use the final accepted module/test names, but filters must
match and run the stated controls:

```text
cargo test --locked -p pangopup-index mask
cargo test --locked -p pangopup-build mask_production
cargo test --locked -p pangopup-build source_fingerprint
cargo test --locked -p pangopup-build --features mask-qualification mask_production
cargo check --locked -p pangopup-build
make lint
make test
make spec
```

## Decisions

### 1. Re-encode the canonical stream; never promote a benchmark member

- Considered: copy/reheader `domains.pgm`; decode that candidate into a new
  file; derive production bytes directly from authenticated canonical JSONL.
- Decision: derive a separate production member from canonical JSONL.
- Why: the candidate is evidence for a layout choice, not a trusted runtime
  supply chain. Independent construction prevents benchmark parsing and
  accidental format compatibility from becoming production dependencies.

### 2. Prepartition domain postings by strand

- Considered: preserve the candidate's mixed posting count and per-query
  scratch/sort/two-pass decode; add larger parallel strand indices; retain the
  same 16 bytes while splitting the count into two `u16` values.
- Decision: store plus postings followed by minus postings and record both
  counts in the existing 16-byte domain record.
- Why: upstream rank is already canonical and complete-domain certification
  proves cardinalities. The provider can copy each result once in final order
  with no scratch vector, sort, or second decode and no member-size increase.

### 3. Keep the mask trait local to the production index module

- Considered: add a general trait to `pangopup-core`; expose only a concrete
  reader; define the typed trait beside the mask format.
- Decision: define `MaskProvider` and its data types in
  `pangopup-index::mask`.
- Why: model inference needs a replaceable typed boundary, but changing the
  existing shared core would falsely churn future SNV/reference builder
  provenance.

### 4. Make runtime open cheap and installation certification complete

- Considered: hash and decode all 6.7 MB on every open; trust every payload
  byte; validate bounded structure at open and validate records when touched.
- Decision: cheap open authenticates bounded metadata/structure from held
  descriptors; the one-time builder/certifier owns full hashing and semantic
  replay; queries validate everything they touch.
- Why: startup and restart should not become full verification, while
  corruption still produces a typed, local failure. Future installation must
  verify hashes before activation.

### 5. Keep production construction private for this slice

- Considered: add another public maintainer command now; use an undocumented
  ordinary command; expose a branch only with the existing private feature.
- Decision: use the existing `mask-qualification` feature and keep the branch
  absent from ordinary builds/help.
- Why: the open maintainer-interface issue must be fixed before widening the
  supported command catalog. The retained one-time invocation remains recorded
  for reproducibility without becoming an end-user promise.

### 6. Qualify once after code review

- Considered: let implementation repeatedly tune against the full private
  source; run a long full verifier in normal tests; accept the implementation
  on independent miniature evidence, then authorize one retained complete run.
- Decision: code review precedes one coordinator-authorized complete run; the
  same reviewer examines the result afterward.
- Why: this keeps development fast, prevents accidental production-source
  dependence, preserves the expensive evidence, and makes a failure useful
  rather than an invitation to retry until it passes.

## Dependencies

Ticket 012 (mask semantics/selection) and Ticket 013 (artifact provenance),
both complete.

## Notes

- The retained Ticket 012 path is evidence owned by the user. Do not modify,
  rename, clean, compress, or delete it.
- Do not create the production output directory before code-review approval.
- Do not kill or attribute the two pre-existing Ticket 008 upload-test orphan
  processes to this ticket. Record process state around the one retained run.
- Keep temporary test/build output under `target/`; keep the retained successful
  production result only under the explicit data path.
- This ticket creates a local qualified asset, not a published GitHub release.

## Coordinator Authorship

Coordinator: `/root`

Drafted from the shipped Ticket 013 outcome, the rolling frontier, ADR 0011,
the retained Ticket 012 evidence, and a read-only filesystem/code audit at base
`9fd8302`. No production input or candidate member was opened while drafting.

## Independent Ticket Review

Reviewer: `/root/ticket013_design_review3`

Initial verdict: `REJECTED`.

The reviewer confirmed the format arithmetic, 16-bit strand counts, 1,160-byte
constructor-read requirement, candidate isolation, held-descriptor API, and
private-feature boundary were feasible. It found three Major gaps:

1. the inventory and performance workload were not byte-pinned or required to
   be hashed and consumed through the same held handles;
2. the benchmark omitted exact nearest-rank indices, measurement windows, and
   executable/host evidence; and
3. semantic certification could publish a bundle before latency/resource
   qualification, leaving a failed performance result at a production path.

It also asked the contract to distinguish manifest identity from member
digests, pin the compatibility corpus, and state the exact unchanged
SNV/reference fingerprints.

Coordinator disposition: all findings accepted. The revised contract pins the
inventory, performance workload, corpus, and prior fingerprints; requires
same-handle authentication/consumption; reproduces Ticket 012's exact
quantile/page arithmetic and measurement windows; identifies the executable
and host; and permits no-replace runtime publication only after every semantic
and resource gate passes. Failures are quarantined as explicitly non-runtime
evidence.

First re-review verdict: `REJECTED`.

The reviewer found that query selection was not fixed even though quantile
indices were, so a runner could time an easier ordering/subset. It also found
that beginning builder-heap tracking after input preflight could hide retained
preflight allocations in the baseline. Finally, it corrected the description
of the aggregate compatibility identity.

Coordinator disposition: all findings accepted. Every round now cycles the
exact ordered 1,000-query manifest for 10 warmup and 100 measured cycles with
no substitution. Builder-heap tracking begins before any input open,
authentication, or preflight and covers report-value assembly; bounded report
serialization occurs after the peak is frozen. The aggregate, manifest, and
case identities are named separately.

Second re-review: `ACCEPTED AS READY`.

The same reviewer independently recomputed contract identity
`ca7341aa36c6b0f55821d3a315f6d9ac2accdb77bd0ad93baebb662b72176a28`
at the pinned base and found no remaining Major or Minor findings. It verified
the exact workload order, complete builder-heap preflight window, distinct
aggregate/manifest/cases identities, publication/quarantine rule, provenance,
format, and qualification contracts. The review was read-only and accessed no
production payload.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable. The authorized complete build is a local,
reversible artifact-generation step under the explicit data path, not a
network operation, publication, release, deployment, or mutation of an
upstream source.

## Coordinator Final Check

Coordinator: pending
