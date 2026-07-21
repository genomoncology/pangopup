# 003 — Full-corpus index build and offline certification

Status: proposed

## Why

Ticket 002 selects and proves a private index layout on a bounded corpus. A
usable Pangopup data asset still requires deterministic construction from all
19,913 source members, independent GRCh38 reference certification, complete
offline verification, and atomic publication without loading billions of loci
into heap memory.

This slice produces that complete installed-form bundle and its build evidence.
It deliberately stops before the public lookup trait and user-facing query CLI.

## Scope

- Promote the exact format selected and documented by Ticket 002; do not reopen
  the codec decision unless full-corpus evidence demonstrates that its declared
  bounds or hosting constraints are impossible.
- Extend `pangopup-build` with:

  ```text
  pangopup-build build --source <DIR> --reference <FASTA> --output <BUNDLE>
  pangopup-build verify <BUNDLE>
  ```

  Build inputs are explicit and read-only. Neither command downloads data or
  discovers a home directory.
- Stream source members through the Ticket 001 visitor and selected writer.
  Canonicalize descending genes without holding the complete corpus; peak heap
  remains proportional to one source gene plus bounded directories and writer
  state. Use `u64` for corpus counts, offsets, pair counts, and size arithmetic.
- Encode every ordinary locus, all source-gene overlaps, all five contiguous
  segments contributed by the two source genes that contain the three real
  gaps, and all 30 `REF=N` exceptions. The
  result is deterministic regardless of filesystem enumeration order.
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
- Record the supplied FASTA's byte SHA-256 separately from a canonical required
  sequence-set SHA-256. The latter is computed in the accession order above over
  repeated `u64_le(accession_len) || accession || u64_le(sequence_len) ||
  uppercase_sequence` frames after removing FASTA whitespace. Reject duplicate
  or missing required accessions and non-IUPAC sequence bytes. Extra records are
  ignored for certification but their accessions are listed in the report. An
  existing `.fai` may accelerate access only after its FASTA byte identity and
  every required accession/length fact have been validated.
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
- Embed format version, builder version/commit, source DOI and published archive
  metadata, and a new full-source member-set SHA-256 over all 19,913 accepted
  members. Reuse Ticket 002's sorted-name framing algorithm, not its benchmark-
  subset digest value, and record both the full member count and digest. Also
  record source counts, GRCh38 reference identity, masked/window parameters,
  section sizes/counts, attribution identity, and provenance in each data
  member. Keep an external deterministic `manifest.json` that hashes every data
  member and never hashes itself. Data members contain neither their own hash
  nor the manifest-derived bundle identity. Serialize the manifest as RFC 8785
  canonical JSON with no timestamp or bundle-identity field; the SHA-256 of its
  exact bytes is the bundle identity. Include the CC BY notice in the logical
  bundle contract.
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
- `verify` streams every section and proves global ordering, section/rank/count
  invariants, record decode completion, hashes, source totals, overlap index,
  gap segments, and exception counts. Ordinary reader open remains cheap and is
  not changed into a full verifier.
- Add a small synthetic source/reference fixture for `make spec` that can prove
  build, verify, reference mismatch, failed-publication preservation, and
  deterministic repeated output without committing a large FASTA or bundle.
- Run one non-gate complete build using `PANGOPUP_SOURCE_DIR` and
  `PANGOPUP_GRCH38_FASTA`. Retain
  `planning/artifacts/003-full-index-build.md` with exact input identities,
  command shape, output hashes/sizes, counts, wall/user CPU, peak RSS, and
  verification result; never retain the generated full bundle in Git.
- Update `architecture/index.md`, `architecture/source-data.md`, `README.md`,
  and `planning/frontier.md` with shipped behavior and measured facts. Amend the
  Ticket 002 format ADR only if a full-corpus bound invalidates it, and return
  that material change to Ticket 002's decision rationale explicitly.
- Excluded: release upload/download, XDG installation, public score-provider
  API, end-user lookup CLI, HTTP, model/reference runtime assets, inference, and
  result caching.

## Success Checklist

- Two builds from identical checked inputs produce byte-identical installed
  bundles and manifests. The manifest hashes every data member, its own exact
  byte hash is the bundle identity, and no hash is self-referential.
- The manifest records all 19,913 accepted source members and the full observed
  member-set SHA-256, distinct from the published ZIP MD5 and Ticket 002's
  selected benchmark-subset digest.
- Synthetic specs prove successful build/verify, precise ordinary-reference
  mismatch failure, corrupt-bundle verification failure, idempotent success for
  an identical verified destination, and untouched different/invalid existing
  destinations.
- The complete build accounts for exactly 19,913 genes, 4,099,255,665 rows,
  1,366,418,555 gene-loci, 10,073 ascending and 9,840 descending members,
  19,916 segments, 3 gaps, 50,002 omitted bases, 30 `REF=N` loci, 9 omit-A and
  21 omit-T shapes, plus every encoded overlap required by the source.
- Every ordinary reference agrees with the pinned GRCh38 FASTA or the ticket
  stops with a documented mismatch rather than publishing. The retained report
  distinguishes source `N` exceptions from ordinary-reference certification.
- Full offline verification succeeds on the produced bundle and independent
  mutation tests cover header, manifest, directory, rank, payload, exception,
  and hash corruption.
- The canonical logical record count/digest computed before encoding equals the
  independently decoded complete-bundle count/digest.
- Peak heap is bounded by one source gene plus compact directories/writer state;
  the retained report includes measured peak RSS and explains any component
  larger than the largest input member.
- Output installed size and transport-compressed size are recorded. If one
  release archive would approach the hosting per-asset ceiling, the report
  recommends split transport members without changing query semantics.
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

- Ticket 002 complete, with accepted format ADR 0006 and selected writer/reader
  code present on `main`. If Ticket 002 concludes that Tabix wins and adopts no
  product format, this draft remains blocked and must be replaced rather than
  dispatched.

## Notes

- This is a reviewed dependency-gated draft. Do not mark it `ready` or dispatch
  it until Ticket 002 ships; then re-read current code/docs and revalidate every
  assumption with an independent ticket review.
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

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending
