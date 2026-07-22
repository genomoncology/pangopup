# 003 — Full-corpus index build and offline certification

Status: complete

## Why

Ticket 002 selects and proves a private index layout on a bounded corpus. A
usable Pangopup data asset still requires deterministic construction from all
19,913 source members, independent GRCh38 reference certification, complete
offline verification, and atomic publication without loading billions of loci
into heap memory.

This slice produces that complete installed-form bundle and its build evidence.
It deliberately stops before the public lookup trait and user-facing query CLI.

## Scope

- Preserve Ticket 002's fixed 11-byte v1 index member exactly: its 320-byte
  header, segment/tree/payload/exception sections, and cheap `IndexReader` open
  contract do not gain provenance or trailing sections. The production bundle
  wraps that unchanged `scores.pgi` member with an external manifest and notice.
  Do not reopen the codec decision unless full-corpus evidence demonstrates that
  its declared bounds or hosting constraints are impossible.
- Extend `pangopup-build` with:

  ```text
  pangopup-build build --source <DIR> --reference <FASTA_OR_GZIP> --output <BUNDLE>
  pangopup-build verify <BUNDLE>
  ```

  Build inputs are explicit and read-only. Neither command downloads data or
  discovers a home directory.
- Retain `write_index(&[InputLocus])` and `prototype_roundtrip` as bounded-fixture
  APIs only. Add a production incremental/spooling writer that accepts complete
  genes in ascending Ensembl-ID order, canonicalizes at most one ascending or
  descending gene in memory, writes fixed payload/reference work to scratch
  files, and retains only compact segment/tree/member directories in heap. Once
  counts and offsets are known, stream the final header/directories and scratch
  payload into `scores.pgi`; never construct an artifact-sized `Vec`. Use `u64`
  for corpus counts, offsets, pair counts, and size arithmetic.
- Encode every ordinary locus, all source-gene overlaps, all five contiguous
  source segments contributed by the two genes that contain the three real
  gaps, and all 30 `REF=N` exceptions. Removing those exception positions from
  ordinary payload creates 19,945 encoded index segments, distinct from the
  19,916 source segments. The result is deterministic regardless of filesystem
  enumeration order.
- Certify every ordinary source reference against these required primary
  sequences in NCBI RefSeq GRCh38.p14 assembly `GCF_000001405.40`:
  `NC_000001.11`, `NC_000002.12`, `NC_000003.12`, `NC_000004.12`,
  `NC_000005.10`, `NC_000006.12`, `NC_000007.14`, `NC_000008.11`,
  `NC_000009.12`, `NC_000010.11`, `NC_000011.10`, `NC_000012.12`,
  `NC_000013.11`, `NC_000014.9`, `NC_000015.10`, `NC_000016.10`,
  `NC_000017.11`, `NC_000018.10`, `NC_000019.10`, `NC_000020.11`,
  `NC_000021.9`, `NC_000022.11`, `NC_000023.11` (X), `NC_000024.10`
  (Y), and `NC_012920.1` (mitochondrial). The builder owns the explicit
  chromosome/accession alias table. `REF=N` loci are counted
  and preserved as source exceptions, not treated as reference mismatches.
  Any ordinary mismatch fails publication and reports bounded examples plus the
  total mismatch count.
- Accept plain FASTA and ordinary single-member gzip FASTA. In one sequential
  pass, hash the supplied bytes, decompress if needed, use the first whitespace-
  delimited header token as the accession, reject duplicate/missing required
  accessions and non-IUPAC sequence bytes, and write only the 25 normalized
  uppercase primary sequences to a builder-owned disk scratch member with a
  25-entry offset/length map. Do not consume an external `.fai` or `.gzi` in
  this ticket. Accepted sequence bytes are ASCII
  `A,C,G,T,R,Y,S,W,K,M,B,D,H,V,N` in either case; FASTA sequence lines must be
  nonempty and contain no spaces or tabs, with LF or CRLF line endings. Gene-
  order REF checks use bounded reads from that scratch member. Put reference and
  payload scratch beneath the unique sibling staging directory and remove the
  entire staging directory after every handled failure as well as after an
  `already_present` result; no partial work survives. Extra records are
  ignored for certification but their 680 accessions are listed in the retained
  report. Record the supplied file byte SHA-256 separately from a canonical
  required sequence-set SHA-256 computed in the accession order above over
  repeated `u64_le(accession_len) || accession || u64_le(sequence_len) ||
  uppercase_sequence` frames.
- Write to a new staging directory on the destination filesystem, `sync_all`
  each member and the deterministic manifest, sync the staging directory, run
  complete offline verification, rename atomically, then sync the containing
  directory where the platform supports directory sync. Treat supported sync
  failures as build failures; document any platform whose directory-sync limit
  weakens crash durability. `<BUNDLE>` is immutable: if absent, rename the
  verified staging directory into that path; if it already contains the same
  independently verified bundle identity, return success without mutating it;
  if it has a different identity or is invalid, fail and leave it untouched.
  Replacement/version selection belongs to the later installer. A failed build
  or verify therefore never replaces an existing complete bundle.
- The bundle directory contains exactly three regular files and no symlinks:
  `scores.pgi`, `NOTICE`, and `manifest.json`. `NOTICE` is the byte-exact notice
  embedded from the repository at compile time. Keep all bundle provenance out
  of fixed-v1 `scores.pgi`. The RFC 8785 canonical manifest has these closed,
  required keys and types (unknown keys are rejected):

  ```text
  schema: "pangopup.bundle.v1"
  index_format: "pangopup.fixed11.v1"
  builder: {version: string, source_sha256: "sha256:<64 lowercase hex>"}
  source: {title: string, creators: [string], doi: string,
    archive_name: string, published_archive_size: u64,
    published_archive_md5: "md5:<32 lowercase hex>",
    observed_member_count: u64, observed_members_sha256: "sha256:<64 lowercase hex>",
    masked: bool, window: u32}
  reference: {assembly: "GRCh38.p14", assembly_accession: "GCF_000001405.40",
    input_compression: "none"|"gzip", input_size: u64,
    input_sha256: "sha256:<64 lowercase hex>",
    sequence_set_sha256: "sha256:<64 lowercase hex>",
    aliases: [{contig: string, accession: string, length: u64}],
    extra_record_count: u64, extra_accessions_sha256: "sha256:<64 lowercase hex>"}
  counts: {genes: u64, source_rows: u64, gene_loci: u64,
    ascending_members: u64, descending_members: u64, source_segments: u64,
    index_segments: u64, gap_transitions: u64, omitted_bases: u64,
    n_ref_loci: u64, n_omit_a: u64, n_omit_t: u64}
  logical_source: {records: u64, sha256: "sha256:<64 lowercase hex>"}
  logical_decoded: {records: u64, sha256: "sha256:<64 lowercase hex>"}
  members: [{path: string, size: u64, sha256: "sha256:<64 lowercase hex>",
    media_type: string}]
  attribution: {notice_path: "NOTICE", license: "CC-BY-4.0",
    transformed: true}
  ```

  `members` is sorted by path and contains exactly `NOTICE` and `scores.pgi`.
  `aliases` uses canonical contig order. Member media types are exactly
  `application/vnd.pangopup.fixed11` and `text/plain; charset=utf-8`.
  `extra_accessions_sha256` hashes accession names in sorted UTF-8 byte order as
  repeated `u64_le(name_len) || name` frames.
  `source_sha256` is a build-time digest over sorted workspace-relative UTF-8
  paths for the root `Cargo.toml`, `Cargo.lock`, and every `Cargo.toml`/`.rs`
  file in `pangopup-core`, `pangopup-index`, and `pangopup-build`, using repeated
  `u64_le(path_len) || path || u64_le(file_len) || file_bytes` frames. The
  retained report separately records the base Git commit. Hash every
  non-manifest bundle file; the manifest never hashes itself and contains no
  bundle-ID or timestamp. The SHA-256 of its exact canonical bytes is the bundle
  identity exposed as `sha256:<hex>`.
- Own the closed manifest structs, canonical parser/serializer, member-set
  validation, and cheap bundle-open validation in `pangopup-index`; own source
  ingestion, member hashing, full verification, scratch work, and publication
  in `pangopup-build`. The cheap path checks schema, names, types, sizes, and
  index compatibility without rereading all member bytes; it does not duplicate
  the full verifier or silently claim checksum verification.
- Record source DOI and published archive metadata plus a new full-source
  member-set SHA-256 over all 19,913 accepted
  members. Reuse Ticket 002's sorted-name framing algorithm, not its benchmark-
  subset digest value, and record the full member count and digest in the
  manifest and report.
- Before encoding, hash an independent canonical logical stream; after building,
  decode the complete bundle back to the same stream and require record count
  and SHA-256 equality. Its exact UTF-8 lines, newline-terminated and ordered by
  ascending Ensembl gene, canonical contig order `chr1` through `chr22`, `chrX`,
  `chrY`, `chrM`, position, then alternate `A,C,G,T`, are:
  `O<TAB>gene<TAB>contig<TAB>pos<TAB>ref<TAB>alt<TAB>gain_hundredths<TAB>gain_pos<TAB>loss_hundredths<TAB>loss_pos`
  for ordinary records and the same with leading `N`, literal reference `N`,
  plus a final `omitted_alt` field for exceptions. Scores are unsigned integer
  magnitudes `0..100` (the leading `O`/`N` and field order carry gain/loss
  semantics); positions are signed decimal integers; zero has one representation.
  Retain the source-side and decoded-side digest/counts.
- `verify` rejects missing, extra, substituted, non-regular, or symlink bundle
  members; validates the closed manifest and every member size/hash; then streams
  every index section and proves global ordering, section/tree/reserved-field/
  count invariants, record decode completion, reconstructable source totals,
  overlap index, source/index segment counts, and exception counts. The source
  direction split remains provenance whose checked sum is verified against the
  decoded gene count; canonical fixed-v1 does not preserve enough information to
  reconstruct the split independently. Ordinary reader open remains cheap and
  is not changed into a full verifier.
- Machine output is exactly one JSON line. Successful build emits `status` (`built` or
  `already_present`), `bundle_id`, and all manifest `counts`; successful verify
  emits `status: "verified"`, `bundle_id`, and `members_verified: 2`. Exit 0
  covers these successes. Every failure, including CLI usage, suppresses default
  prose usage and prints exactly
  `{"status":"error","code":string,"message":string,"details":object|null}`
  to stderr with no stdout: exit 2 uses `CLI_USAGE` or `UNSUPPORTED_INPUT`, and
  exit 1 uses a stable typed `SOURCE_*`, `REFERENCE_*`, `BUNDLE_*`, `IO`, or
  `PUBLICATION` code. `REFERENCE_MISMATCH` details are exactly
  `{mismatch_count:u64,examples:[{gene:string,contig:string,pos:u64,expected:string,observed:string}]}`;
  examples are capped at 20 and ordered by gene, contig, position, expected,
  then observed. Other error details are closed, code-specific serializable
  structs (or null), never ad-hoc maps; their exact shapes are asserted in tests.
- Add a small synthetic source/reference fixture for `make spec` that proves
  plain and ordinary-gzip FASTA build, verify, deterministic repeated output,
  reference mismatch, missing/duplicate accession, invalid sequence byte,
  read-only inputs, scratch cleanup, and immutable destination behavior. Unit/
  integration tests additionally cover missing/extra/substituted/symlink member,
  member hash and NOTICE corruption, concurrent absent-destination publication,
  tree/reserved-field corruption, and a synthetic scale input that detects
  cross-gene or artifact-sized heap accumulation.
  Member-hash mutations leave the manifest unchanged and must fail at the outer
  hash check. Header/tree/payload/exception semantic mutations must recompute the
  mutated `scores.pgi` member size/hash and recanonicalize `manifest.json`, so
  they reach and prove the corresponding inner verifier rather than being
  intercepted by `BUNDLE_MEMBER_HASH`.
- Run one non-gate complete build using `PANGOPUP_SOURCE_DIR` and
  `PANGOPUP_GRCH38_FASTA`. Retain
  `planning/artifacts/003-full-index-build.md`. Record base commit, builder source
  digest, compiler, OS/hardware, exact commands, input identities, elapsed/user/
  system CPU, peak-RSS command/method, scratch peak bytes, installed member
  hashes/sizes, source and encoded segment counts, both logical digest/count
  pairs, bundle ID, and verification result; never retain the generated bundle.
  Measure deterministic transport size with GNU tar 1.35 and Zstandard CLI 1.5.5:

  ```text
  tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --mode=0644 --format=posix \
    --pax-option=delete=atime,delete=ctime -cf - manifest.json NOTICE scores.pgi \
    | zstd -9 --threads=1 --no-progress -o pangopup-hg38-snvs-masked-v1.tar.zst
  ```

  Record the transport byte size and SHA-256 and delete it after evidence is
  retained.
- Update `architecture/index.md`, `architecture/source-data.md`, `README.md`,
  and `planning/frontier.md` with shipped behavior and measured facts. Amend the
  Ticket 002 format ADR only if a full-corpus bound invalidates it, and return
  that material change to Ticket 002's decision rationale explicitly.
- Excluded: release upload/download, XDG installation, public score-provider
  API, end-user lookup CLI, HTTP, model/reference runtime assets, inference, and
  result caching.

## Success Checklist

- Two builds from identical checked inputs produce byte-identical installed
  bundles and manifests. The manifest hashes both non-manifest members, its own
  exact byte hash is the bundle identity, and no hash is self-referential.
- The manifest records all 19,913 accepted source members and the full observed
  member-set SHA-256, distinct from the published ZIP MD5 and Ticket 002's
  selected benchmark-subset digest.
- Synthetic specs and integration tests prove the complete FASTA, manifest,
  publication, cleanup, output/exit-code, deterministic-build, and corruption
  matrix named in Scope.
- The complete build accounts for exactly 19,913 genes, 4,099,255,665 rows,
  1,366,418,555 gene-loci, 10,073 ascending and 9,840 descending members,
  19,916 source segments, 19,945 encoded index segments, 3 gaps, 50,002 omitted
  bases, 30 `REF=N` loci, 9 omit-A and 21 omit-T shapes, plus every encoded
  overlap required by the source.
- Every ordinary reference agrees with the pinned GRCh38 FASTA or the ticket
  stops with a documented mismatch rather than publishing. The retained report
  distinguishes source `N` exceptions from ordinary-reference certification.
- Full offline verification succeeds on the produced bundle and independent
  mutation tests cover header, manifest, directory, tree, reserved fields,
  payload, exception, member-set, NOTICE, and hash corruption.
- The canonical logical record count/digest computed before encoding equals the
  independently decoded complete-bundle count/digest.
- Peak heap is bounded by one source gene plus compact directories/writer state;
  payload and normalized reference scratch are disk-backed. The retained report
  includes measured peak RSS and scratch peak and explains any heap component
  larger than the largest input member.
- Output installed size and transport-compressed size are recorded. If one
  release archive would approach the hosting per-asset ceiling, the report
  recommends split transport members without changing query semantics.
- Successful `build`/`verify` output and every failure use the exact JSON/exit-
  code contract in Scope; mismatch examples are deterministic and capped at 20.
- `make lint`, `make test`, and `make spec` pass without external datasets.

## Decisions

### Reference certification is a release prerequisite

- Consideration: the source publisher says hg38 but does not identify the exact
  FASTA, and internal REF consistency cannot prove external coordinates.
- Options: trust the label; sample positions; compare every ordinary locus to a
  pinned build-qualified reference.
- Trade-offs: full comparison adds build I/O but happens offline once; weaker
  approaches permit a silently mislabeled permanent asset.
- Decision: certify every ordinary locus against RefSeq GRCh38.p14 and fail
  publication on any mismatch. Preserve `REF=N` separately.

### Build a private reference scratch index

- Consideration: the actual NCBI reference is ordinary gzip with 705 records and
  no random-access index, while source validation proceeds in gene order.
- Options: require an operator-created uncompressed `.fai`; hold chromosomes in
  RAM; accept plain/gzip input and build bounded disk scratch internally.
- Trade-offs: external preparation makes the command brittle; RAM breaks the
  memory bound; internal scratch costs sequential disk I/O once but keeps inputs
  read-only and behavior reproducible.
- Decision: accept plain or ordinary-gzip FASTA, sequentially validate/hash it,
  write only the 25 required uppercase sequences to private disk scratch, and
  delete scratch on every exit. External `.fai`/`.gzi` support is deferred.

### Keep fixed-v1 data separate from bundle provenance

- Consideration: the selected reader requires an exact 320-byte fixed-v1 header
  and rejects trailing bytes, while a distributable bundle needs rich source,
  reference, attribution, and member identity.
- Options: version the index envelope; append unsectioned metadata; preserve the
  measured index and bind it with a closed external manifest and notice.
- Trade-offs: an envelope change invalidates Ticket 002's measured reader;
  trailing metadata is malformed; a manifest adds one small open-time file while
  keeping the hot data format stable.
- Decision: `scores.pgi` remains exact fixed-v1. `manifest.json` owns all bundle
  provenance and hashes `scores.pgi` plus `NOTICE`; the manifest's own canonical
  hash is the bundle identity.

### Replace accumulation with a production spooler

- Consideration: the fixture API sorts and encodes the entire artifact in heap
  and therefore cannot build 1.366 billion loci.
- Options: generalize that `Vec` API; require source pre-sorting into final file
  order; add a gene-bounded incremental writer with disk-backed payload scratch.
- Trade-offs: retaining the fixture API keeps small tests simple; production
  spooling adds temporary I/O but avoids artifact-sized memory and preserves the
  chosen byte layout.
- Decision: keep the prototype API bounded and add a separate production writer
  that holds at most one gene plus compact directories in memory and streams the
  final fixed-v1 member from scratch.

### Full verification is offline, startup validation is cheap

- Consideration: hashing and scanning a multi-gigabyte bundle on every process
  start defeats mmap startup and page selectivity.
- Options: trust all bytes; full-scan every open; perform cheap structural open
  checks plus an explicit complete verifier before publication/on demand.
- Trade-offs: the split requires two validation levels but gives both corrupt
  artifact detection and fast startup.
- Decision: builder publication requires full `verify`; runtime open checks only
  identity, sizes, versions, section bounds/order, and small directories, while
  lookup validates touched payload.

### Publish immutable output atomically

- Consideration: readers may mmap a bundle while another process installs a new
  one, and in-place mutation can produce mixed versions or `SIGBUS`.
- Options: overwrite in place; write temp and rename without verification;
  stage, flush, fully verify, then atomically publish a new immutable identity.
- Trade-offs: staging temporarily requires additional disk space but makes
  failures and concurrent readers safe.
- Decision: never mutate a published member. Verify a new staged artifact and
  atomically publish it on the same filesystem.

### Determinism is byte-level

- Consideration: stable logical records are insufficient for reproducible
  release hashes when directory order, timestamps, or compression metadata vary.
- Options: accept logical equivalence; normalize only manifests; require
  byte-identical installed output from identical inputs.
- Trade-offs: byte determinism constrains metadata and writer ordering but makes
  provenance, mirrors, and independent verification much simpler.
- Decision: identical inputs and builder identity must produce byte-identical
  installed bundles; wall-clock timestamps never enter hashed members.

## Dependencies

- Ticket 002 complete in commits `72b27c8`/`da639ae`, with fixed 11-byte ADR
  0006 and its selected reader plus bounded prototype writer present on `main`.

## Notes

- Ticket 002 is shipped. Do not dispatch until the fresh dependency-time review
  recorded below approves these revised production-build contracts.
- For the retained lab run, the operator supplies paths in the named environment
  variables and the harness expands them into the command's explicit `--source`
  and `--reference` flags. The command itself performs no environment discovery.
  Do not download the inputs, commit them, or record local paths.
- The fixture reference may be synthetic and tiny, but must be plainly labeled;
  it cannot be cited as GRCh38 evidence.
- Evidence in this ticket is illustrative unless explicitly named as a retained
  artifact. Public files contain no machine paths or sibling-project references.
- Run exactly `make lint`, `make test`, and `make spec` from repository root.

## Independent Ticket Review

Reviewer: `next_ticket_set_review` (independent, read-only packet review)

Initial result: changes required. The coordinator accepted every finding:
published/archive identity is separated from actual input identity; an
independent canonical logical-stream digest proves full exactness; the manifest
hash graph is non-self-referential; the required RefSeq accessions and FASTA
identity contract are exact; segment wording and crash durability are explicit.
Immutable publication has defined same/different/invalid existing-destination
behavior.

Final packet re-review: approved with no remaining findings as a dependency-
gated `proposed` draft only. It must receive a fresh independent review against
the implementation and ADR produced by Ticket 002 before becoming `ready`.

Fresh dependency-time reviewer: `ticket_003_review` (independent, read-only).

Initial result: changes required. The coordinator accepted every finding:

- plain and ordinary-gzip FASTA now have a builder-owned bounded disk-scratch
  preparation path and failure/cleanup tests; external indexes are deferred;
- the exact fixed-v1 member remains unchanged and the production bundle has a
  closed, non-self-referential manifest plus hashed NOTICE contract;
- the prototype accumulator is explicitly replaced by a production incremental
  spooler with a synthetic scale proof;
- the full-source rescan separately pins 19,916 source segments and 19,945
  encoded segments after the 30 `REF=N` positions are removed from ordinary
  payload;
- output/error, publication race, member corruption, report, and deterministic
  transport evidence are exact.

Final dependency-time re-review: approved with no remaining findings. The
reviewer verified fixed-v1 compatibility; the closed manifest and crate
ownership; FASTA, scratch, and streaming-memory contracts; separate source and
encoded segment counts; exact failure JSON; inner-verifier mutation tests;
atomic publication; and reproducible full-run/transport evidence.

## Implementation Evidence

Developer: `ticket_003_development`

- Added the separate gene-bounded `StreamingIndexWriter`, closed RFC 8785
  bundle manifest, cheap `BundleOpen`, complete logical decode visitor, explicit
  reference scratch preparation/certification, typed JSON CLI errors, full
  verifier, and Linux atomic no-replace publication. Fixed-v1 bytes remain the
  unchanged 320-byte header plus segment/tree/payload/exception sections.
- Added synthetic plain/gzip build fixtures, executable build/verify specs, and
  integration coverage for deterministic/read-only builds, FASTA failures,
  cleanup, immutable/concurrent publication, closed manifests, missing/extra/
  substituted/symlink members, outer hashes, recanonicalized inner corruptions,
  exact NOTICE, reconstructed counts, and scaled disk spooling.
- Retained the complete non-gate run in
  `planning/artifacts/003-full-index-build.md`. The remediation recertification
  used builder source digest
  `sha256:14b086f124c5fae4a720db7d35b0c120a50372f81bd98265f389e95b13adcf24`,
  certified all required counts, zero ordinary-reference mismatches, equal
  4,099,255,665-record logical streams
  (`sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31`),
  bundle ID `sha256:bce0bb49ba8a3f303661967a7a86362da66013fd94c3ae32ed27a9685d3b5260`,
  and an independently successful full verify. The report explicitly marks all
  earlier Ticket 003 source/bundle identities superseded.
- The exact deterministic transport was 1,935,000,209 bytes with SHA-256
  `3e87d80fdad963ca6ffca646393b8bb3955214b77cd8b7f1782e48d039aba751`;
  retained evidence recommends split release transport for headroom while
  preserving installed fixed-v1 semantics. Generated bundles, copies, scratch,
  timing logs, and the archive were deleted after evidence capture.
- Remediation developer gates from repository root: `make lint` passed; `make
  test` passed all workspace tests (including 10 full-bundle integration tests,
  the dedicated allocator/RSS regression, and explicit handled-cleanup test);
  `make spec` passed all 25 executable specifications; and the overflow CLI
  regression passed in both debug and optimized release profiles.

## Adversarial Code Review

Reviewer: independent code reviewer — changes required

The reviewer returned nine material findings. Developer dispositions:

1. **Unchecked untrusted count arithmetic.** Remediated: every arithmetic
   relationship among manifest counts uses `checked_add`/`checked_mul`; overflow
   returns typed `BUNDLE_COUNTS` JSON. The same subprocess CLI regression passes
   in debug and optimized release profiles.
2. **Ticket 002 cheap-open compatibility regression.** Remediated: fixed-v1
   runtime open again accepts ordered, non-overlapping section and payload
   ranges with internal gaps while retaining Ticket 002's terminal-coverage
   rules: trailing unsectioned file bytes and an unclaimed payload tail are
   rejected during cheap open. Exact terminal-tail mutations cover both rules.
   The explicit full verifier alone requires the production writer's exact
   section/payload contiguity and rejects non-maximal adjacent segments. A
   physical internal-padding test proves the two validation levels.
3. **Racy portable publication fallback.** Remediated: Linux continues to use
   `renameat2(RENAME_NOREPLACE)`; non-Linux targets safely return a typed
   unsupported publication error and never execute existence-check-plus-rename.
4. **Unreconstructed index segment count and overstated direction proof.**
   Remediated: complete decode reconstructs canonical index segments from
   ordinary gene/contig/position adjacency and compares that result with both
   manifest and fixed-v1 header counts. Source segments remain reconstructable
   after exceptions are merged. Ascending/descending counts are now explicitly
   described as source-only provenance: verification checks their sum without
   claiming it can recover the split from canonical ascending fixed-v1. Any
   edit changes the canonical manifest bundle identity.
5. **Incomplete re-signed mutation matrix.** Remediated with valid mutations of
   tree links, subtree maximum, AVL balance, ordinary reference and score,
   exception allele and score, physical section padding, and canonical segment
   boundary semantics. Outer coverage now includes substituted regular and
   non-regular members in addition to missing/extra/symlink/hash corruption.
6. **Source provenance hash/parse TOCTOU.** Remediated: each compressed source
   member is framed, hashed, decompressed, and parsed through one opened file
   descriptor, and actual bytes read must equal that descriptor's initial
   metadata length. Reference compression detection, hashing, and parsing also
   share one opened descriptor and enforce the same length check.
7. **Bare carriage returns accepted.** Remediated: source TSV and reference
   FASTA accept LF and CRLF only and reject every bare CR. Plain and gzip FASTA,
   plus compressed source-member, regressions cover the rule.
8. **Scale test did not measure memory.** Remediated with a dedicated global
   allocator/current-and-peak-state plus Linux `/proc/self/statm` RSS regression
   over 3,000,000 loci and a 33,000,000-byte spool. The reproducible focused run
   measured 163,840 retained allocator bytes, a 269,472-byte allocator peak
   delta, and a 339,968-byte RSS delta; thresholds fail on retained-locus or
   artifact-sized state. Retained evidence no longer presents file-backed mmap
   RSS as a heap measurement.
9. **Cleanup relied on silent `Drop`.** Remediated: every handled build result
   explicitly removes an unpublished staging directory and returns an `IO`
   failure if cleanup fails. `Drop` remains armed only as a panic/unwind
   fallback, with a focused cleanup-failure regression.

These are material post-review changes and await return to the same independent
reviewer. No approval is claimed here.

Follow-up review found one remaining Ticket 002 compatibility blocker: cheap
open had retained internal padding compatibility but no longer rejected a
terminal unsectioned file tail or terminal unclaimed payload tail. Both terminal
coverage checks are restored without moving internal canonical contiguity back
to startup. Exact cheap-open mutations cover each tail, all gates pass, and the
full corpus was recertified under the final source identity above. This follow-up
also awaits the same independent reviewer's approval.

Final re-review: approved with no remaining findings. The same reviewer
independently confirmed that cheap open permits internal non-overlapping
padding while rejecting both terminal unsectioned file bytes and terminal
unclaimed payload bytes; full verification retains canonical contiguity; the
final builder digest, bundle identity, and transport evidence recompute; all
generated full-run outputs are absent; and `make lint`, `make test`, and all 25
specs pass.
