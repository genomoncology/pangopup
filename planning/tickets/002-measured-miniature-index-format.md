# 002 — Measured miniature index format and lookup kernel

Status: complete

## Why

Pangopup can now validate every published source row exactly, but it still has
no binary index, mmap reader, or honest answer for the cost of opening an index
and returning 1, 10, or 100 distinct SNV scores. The complete entropy analysis
narrows the credible representations; it does not measure executable lookup
code.

This slice implements the smallest lossless writer/reader kernels needed to
compare those representations. It adopts and hardens one v1 private format only
if the evidence shows that a custom format earns adoption; otherwise it records
the failed hypothesis and stops. It does not scale the writer to the full
archive or expose the user-facing lookup CLI.

## Scope

- In `pangopup-index`, implement private experimental codecs/readers for:
  1. the hierarchical sparse direct layout described in `architecture/index.md`;
  2. the 11-byte fixed-locus baseline;
  3. independently compressed sparse blocks using Zstd and LZ4 at 1,024, 2,048,
     and 4,096 loci per block, with raw fallback when compression expands.
- Include bgzip + Tabix as a benchmark-only operational baseline over the same
  logical records. Its primary comparison is in-process: `one-open` keeps one
  Tabix reader/index handle alive, `reopen-plus-query` creates a fresh in-process
  handle per sample, returned source rows are parsed, and subprocess/stdout cost
  is excluded. A separate command-line operational number may be reported but
  cannot select the codec. Tabix is not implemented as Pangopup's product format
  in this ticket; if it beats every custom candidate on the accepted performance
  priorities, selection stops and ADR 0006 records that no custom format has yet
  earned adoption.
- Keep the common logical input identical across candidates: canonical ascending
  gene segments, concrete ordinary reference plus three alternate score records,
  explicit source-gene identity, and a separate `REF=N` exception representation.
  Candidate code must not parse TSV or leak byte offsets through public types.
- Give every candidate an exact generated-artifact round trip and a trusted-input
  benchmark kernel. If the measurement selects a custom format, promote only
  that codec to the product writer/reader and harden it: every offset/count
  product uses checked arithmetic; it rejects wrong magic/version, truncation,
  overlapping or out-of-range sections, invalid allele/value codes, corrupt
  block lengths, incomplete decompression, and invalid exception records.
  Alternative codecs and Tabix do not acquire a production corrupt-input API.
- Every custom candidate uses a per-contig balanced interval tree augmented
  with each subtree's maximum segment end. A point query is `O(log n + k)` for
  `n` gene segments and `k` returned overlaps; no query scans every segment in a
  chromosome or gene. Prove the bound with the complete 19,916-segment directory
  or a deterministic equivalent adversarial corpus containing deeply nested and
  disjoint intervals.
- If a custom format is adopted, constrain unsafe code in its product reader to
  the mmap creation boundary in `pangopup-index` and document its safety
  contract. Never cast mapped bytes to Rust structs. Decode integer fields
  explicitly with a named byte order after cheap structural open validation.
- If a custom format is adopted, extend `pangopup-build` with a developer/admin
  command:

  ```text
  pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>
  ```

  When a custom format is adopted, it streams the checked source fixture through
  the selected writer, opens the produced artifact with the selected reader,
  verifies every ordinary and exceptional source record exactly, prints one
  deterministic summary, and never treats the artifact as a releasable bundle.
  If Tabix wins, do not add this product command; retain the candidate roundtrip
  tests and evidence instead.
- If a custom format is adopted, add `spec/index-prototype.md` covering the
  fixture round trip and one corrupt artifact rejection. The spec creates
  outputs under `target/spec/`; no generated binary artifact is committed.
- Add a custom benchmark target owned by `pangopup-index`, with one documented
  reproducible command, that measures each candidate without the CLI or stdout
  path. Its deterministic workload includes open-only, reopen plus 1/10/100
  distinct queries, one-open plus 1/10/100 distinct queries, same-block hits,
  cross-block hits, gene-filtered hits, all-overlap
  hits, absent alleles, and `REF=N` outcomes. Measure serialization separately.
- Define the measurement modes exactly:
  - `open-only` creates the mmap and performs structural validation inside the
    measured process;
  - `reopen-plus-query` creates a fresh reader for every sample in the same
    process and labels the operating-system cache state;
  - `one-open` reuses one reader for every measured query;
  - primary 1/10/100 workloads contain exactly 1/10/100 requests and return
    exactly 1/10/100 gene-score records; overlap workloads are reported
    separately.
  Instrument logical encoded bytes decoded and unique mapped-file page numbers
  addressed. Report operating-system minor/major fault deltas separately; do
  not describe either measure as exact physical bytes read from storage.
- Correctness gates use only the checked fixture. The retained performance run
  additionally uses an operator-supplied `PANGOPUP_SOURCE_DIR` to construct a
  deterministic stratified lab corpus: all real exception/gap/overlap fixture
  genes plus 128 additional genes selected as 16 evenly spaced filenames from
  each ascending/descending × compressed-member-size-quartile stratum. Record
  the exact selected gene manifest. Record published-source identity separately
  from actual build-input identity: DOI, archive filename, published byte size,
  and the upstream MD5 as publisher metadata; and a deterministic SHA-256 over
  the selected extracted members actually used by this benchmark as observed
  input. Compute that member-set digest in sorted UTF-8 member-name order over
  repeated `u64_le(name_len) || name || u64_le(member_len) || member_bytes`
  frames. Do not claim that `PANGOPUP_SOURCE_DIR` verifies the ZIP's byte
  checksum. If an optional archive path is supplied, hash and report it as a
  separate input.
- Report warm p50/p95/p99 latency, throughput, allocations, artifact bytes,
  logical bytes decoded, unique mapped-file pages addressed per lookup, and
  one-time open cost on named hardware.
  Report page faults where the platform exposes them. Label cold-I/O results
  provisional: this lab corpus is not larger than memory, and only the later
  full artifact can provide defensible cold-page measurements.
- Retain `planning/artifacts/002-index-format-benchmark.md` plus its small query
  and selected-gene manifests. Do not retain generated candidate artifacts or
  raw timing dumps.
- Add `architecture/decisions/0006-index-format-selection.md` recording the measured
  choice, exact private-format invariants, and rejected alternatives. Update
  `architecture/index.md`, `README.md`, and `planning/frontier.md` to reflect
  only what shipped. In particular, make the durable performance proof say that
  this ticket establishes comparative warm behavior and instrumentation, while
  definitive cold-I/O evidence waits for the complete artifact in Ticket 004.
- Excluded: complete-corpus artifact generation, external-reference
  certification, release manifests/assets, stable public provider API,
  user-facing `pangopup lookup`, model execution, HTTP, and result caching.

## Success Checklist

- All candidates round-trip every one of the 6,342 checked source rows without
  floating-point conversion, preserve both overlapping gene records, and
  preserve the two `REF=N` exception shapes without converting them to SNVs.
- If a custom format is adopted, its mutation tests cover every
  header/section/block/value corruption family named in Scope and prove that
  opening validates only structural metadata while lookup validates the bytes
  it touches. Every alternative still passes exact generated-artifact roundtrip.
- If a custom format is adopted, `pangopup-build prototype-roundtrip` produces a
  deterministic selected-format artifact and exact success summary through
  `make spec`; corrupt input exits nonzero with a stable typed reason.
- The benchmark report answers reopen plus 1/10/100 and one-open plus 1/10/100
  for every candidate and the Tabix baseline, clearly separates warm lookup from
  process/open and output costs, and records compiler, commit, CPU, OS, storage,
  corpus/query identities, iterations, and measurement method.
- Format selection follows accepted ADR 0004: correctness first, query latency
  and work second, resident pages third, installed/transport size fourth. The
  sparse direct format remains the default unless a measured speed or
  operational failure displaces it; file size alone cannot do so. If the fair
  in-process Tabix baseline wins on those priorities, the ticket reports that
  result and does not promote a slower custom format.
- Any adopted product reader performs no heap allocation proportional to file
  size and no payload-wide scan during open. Benchmark-only alternative codecs
  are not exported as supported runtime formats.
- Point lookup uses the documented `O(log n + k)` overlap index and passes the
  complete-directory or adversarial-nesting proof without a linear segment scan.
- Ordinary gates require no downloaded corpus and no machine-specific path.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

### Compare only the credible frontier

- Consideration: implementing every conceivable codec would turn a format
  decision into an open-ended compression project.
- Options: direct sparse only; every speculative entropy codec; direct sparse
  plus the fixed and block-compressed baselines already supported by complete
  corpus evidence.
- Trade-offs: one format cannot falsify the design; speculative rANS/FSE work is
  costly and not justified by a 1.59 GiB direct estimate; the three-family set
  spans simplicity, direct access, and compression.
- Decision: compare hierarchical direct, fixed 11-byte, and 1K/2K/4K Zstd/LZ4
  sparse blocks. Defer learned/static entropy codecs unless v1 misses a measured
  deployment requirement.

### Separate correctness, warm performance, and real cold I/O

- Consideration: a tiny checked fixture is excellent for edge-case exactness but
  cannot create trustworthy cold-page behavior.
- Options: benchmark only the fixture; claim cache-dropping makes a small sample
  cold; use the fixture for correctness, a deterministic real lab corpus for
  comparative warm work, and defer definitive cold results to the full bundle.
- Trade-offs: the layered approach produces fewer dramatic numbers but avoids
  pretending synthetic cache state represents deployment.
- Decision: use the three-layer proof named in Scope and label every result by
  layer. No production cold-latency claim leaves this ticket.

### Keep the format private and decode bytes explicitly

- Consideration: mmap speed can tempt native-struct casting and public exposure
  of layout details.
- Options: expose packed structs; use unsafe zero-copy casting; use an opaque
  reader with explicit checked little-endian decoding.
- Trade-offs: casting is concise but couples alignment, endianness, compiler
  layout, and mmap safety; explicit decoding costs a few instructions and keeps
  format evolution and corrupt-input handling controlled.
- Decision: the provider-facing vocabulary stays in core, byte layout stays
  private to `pangopup-index`, and all mapped values are decoded explicitly.

### Retain at most one selected codec, not four product formats

- Consideration: benchmark implementations can accidentally become permanent
  compatibility promises.
- Options: ship every candidate; delete all benchmark code; retain a
  self-contained benchmark target while exporting only the selected reader and
  writer.
- Trade-offs: shipping every format multiplies validation burden; deleting the
  comparison harms reproducibility; keeping alternatives inside the bench target
  preserves evidence without creating runtime contracts.
- Decision: if a custom codec earns adoption, only that codec becomes product
  code. Otherwise none does. Alternative encoders and readers remain private to
  the reproducible benchmark target.

### Use a bounded overlap index

- Consideration: genes overlap and can nest, so a single predecessor search is
  insufficient, while scanning every chromosome segment violates the runtime
  goal.
- Options: linear scan; duplicate loci into one global table; an augmented
  interval tree over the compact gene-segment directory.
- Trade-offs: duplication increases the already large payload; a linear scan has
  an unacceptable worst case; the interval tree adds a small directory field
  and returns all containing segments in output-sensitive time.
- Decision: use a per-contig balanced interval tree with subtree maximum ends,
  giving point lookup `O(log n + k)` and preserving every overlapping gene.

## Dependencies

- Ticket 001 behavior shipped in commit `d84a535`: typed exact scores, streaming
  validated source loci, real fixture, and source provenance.

## Notes

- This ticket is the only draft in this set eligible to become `ready` now.
- Existing complete-corpus facts are in
  `planning/artifacts/2026-07-20-full-dataset-entropy.md`; do not rerun or rewrite
  that analyzer unless a discrepancy is found.
- The operator dataset is optional evidence input via `PANGOPUP_SOURCE_DIR` and
  is never downloaded automatically. Do not record its local path.
- Benchmark evidence is descriptive, not a gate assertion. Tests must not fail
  on nanosecond thresholds.
- If benchmark helpers named here already exist when work starts, reuse them;
  otherwise define them within this ticket's crate boundary.
- Evidence in this ticket is illustrative unless explicitly named as a retained
  artifact. Public files contain no machine paths or sibling-project references.
- Run exactly `make lint`, `make test`, and `make spec` from the repository root.

## Independent Ticket Review

Reviewer: `next_ticket_set_review` (independent, read-only packet review)

Initial result: changes required. The coordinator accepted every finding:
candidate hardening is now limited to the selected product codec; Tabix is a
fair in-process benchmark baseline with a stop condition; all-overlap lookup
has a tested complexity bound;
published ZIP identity is separated from the observed extracted-member digest;
and the 1/10/100/cache/page-work methodology is exact.

Final re-review: approved with no remaining findings. The reviewer confirmed
that the in-process Tabix comparison is fair, the stop condition honors the
speed-first priority, the work is bounded, and this is the only ticket in the
packet eligible to become `ready`.

## Implementation Evidence

Developer: `ticket_002_development`

- Implemented and measured the hierarchical direct, fixed 11-byte, Zstd
  1K/2K/4K, and LZ4 1K/2K/4K candidates plus the in-process Tabix baseline.
  Every custom candidate exactly round-trips all 6,342 checked fixture rows,
  including both overlapping records and both typed `REF=N` exceptions.
- Selected and promoted fixed 11-byte v1 under ADR 0004. On the retained
  9,858,991-locus corpus, the corrected equal-harness fixed kernel had one-open
  p50 latency of 121 ns / 972 ns / 9,949 ns for 1/10/100 records. The ranked
  zero-copy hierarchical direct kernel measured 160 ns / 1,243 ns / 14,749 ns;
  the separately hardened fixed product reader measured 210 ns / 1,964 ns /
  19,588 ns; and Tabix measured 9.372 ms / 97.298 ms / 978.623 ms. The fixed
  selection remains supported and the Tabix stop condition did not trigger.
- Retained the 134-gene selection manifest, 100-query manifest, complete warm
  tables, machine/method details, and framed observed-input SHA-256
  `0852168353c8d309a1850bc64049eb8591b4097c5e8635d1042458c5c37c261b` in
  `planning/artifacts/`.
- Hardened the promoted reader with checked section arithmetic, structural-only
  open validation, touched-payload validation, explicit little-endian decoding,
  mmap as the sole unsafe boundary, and exact open-time tree validation for
  connectivity/coverage, acyclicity, BST order, subtree maxima, contigs, and
  balance. Mutation tests cover magic, version, truncation, overlapping and
  out-of-file sections, counts, reserved bytes, invalid alleles/scores/exceptions,
  low subtree maxima, cycles, and lazy payload rejection. Block length and
  decompression corruption are not applicable to the selected uncompressed
  fixed format; compressed alternatives remain benchmark-only.
- Added the augmented per-contig interval tree and its deterministic
  19,916-segment nested/disjoint proof, the prototype builder/open commands,
  exact fixture verification, and the corrupt-artifact executable spec.
- Final gates on 2026-07-21: `make lint` passed; `make test` passed 23 tests;
  `make spec` passed 10 scenarios. The retained lab corpus fits in RAM, so cold
  I/O, definitive resident-set, and complete installed-size evidence remain
  explicitly deferred to Ticket 004.

## Adversarial Code Review

Reviewer: `ticket_002_code_review` (independent, read-only code review)

Initial result: changes required.

- Material M1: the hierarchical-direct benchmark copied each mapped block into
  a `Vec` and scanned preceding bitmap bits/masks. Disposition: replaced it with
  a zero-copy 4,096-locus mmap kernel using packed six-bit masks, active/pair
  rank checkpoints every 64 loci, and bounded popcount. Exact candidate fixture
  round trips pass, and the real 20-sample corpus selection was rerun.
- Material M2: gene-filtered product and candidate lookup could scan every
  segment for a gene, while the adversarial test covered only unfiltered lookup.
  Disposition: both readers now upper-bound-search `(gene, contig, start)` and
  check one predecessor; gene-filtered exceptions use their full sorted key.
  Product and candidate proofs use a deterministic 19,916-segment directory and
  bound decoded directory work.
- Material M3: open checked link ranges and a weak maximum bound but not the
  complete interval-tree invariants trusted by lookup. Disposition: canonical
  open traversal now proves acyclicity, unique connectivity and coverage,
  strict BST order, exact subtree maxima, contig ownership, and height balance
  without heap allocation proportional to artifact size. Focused low-maximum
  and cycle mutations are rejected at open.
- Material M4: the retained report omitted comparable work metrics, used two
  page aggregation conventions, and used a percentile floor that collapsed p99
  into p95 with 20 samples. Disposition: page instrumentation now consistently
  counts unique 4 KiB mapped artifact pages per complete sample; all candidate
  primary rows retain open/serialization, latency, throughput, allocations,
  logical bytes, pages, and faults; and documented nearest-rank percentiles make
  p99 the maximum at 20 samples.
- Nonmaterial N1–N3: README named the old direct installed form, core accessor
  docs leaked private-format language, and `write_index` still called itself
  direct sparse. Disposition: all three names/boundaries now say fixed 11-byte
  or use representation-neutral core language.

Corrected retained run: fixed selection unchanged. Equal-harness fixed one-open
p50 was 121 / 972 / 9,949 ns for 1/10/100 versus corrected direct at 160 / 1,243
/ 14,749 ns; Tabix remained orders of magnitude slower.

Final re-review: approved. The reviewer confirmed all material findings were
resolved and the corrected selection evidence is credible. One nonblocking
documentation nit remained: the public `IndexReader` comment still called the
selected reader direct sparse. Disposition: corrected it to identify the
private fixed 11-byte format; no behavior changed.
