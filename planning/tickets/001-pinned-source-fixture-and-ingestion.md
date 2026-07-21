# 001 — Pinned source fixture and executable ingestion contract

Status: ready

## Why

The complete-corpus analyzer established the source distribution, but the
product crates still contain no score types, source parser, or reproducible
small corpus. Choosing an mmap byte layout before those semantics are executable
would make the format ticket guess at allele ordering, exact decimal handling,
source direction, and overlapping-gene behavior.

This slice pins a small, attributed excerpt of the real Zenodo source and makes
the source contract executable through typed Rust parsing and a builder-side
inspection command. It ends at validated records and summaries: it writes no
binary index and performs no runtime lookup.

## Scope

- Check in a gzip-compressed, per-gene source fixture under
  `tests/fixtures/pangolin-precompute/` copied without score modification from
  the verified Zenodo archive `md5:679ef0b50e511b6102b4b88fbf811108`:
  - `ENSG00000010610.tsv.gz`, inclusive `chr12:6801301..6801539`, preserving
    the two real omitted single-base transitions;
  - `ENSG00000141499.tsv.gz` (WRAP53), inclusive source positions
    `chr17:7686072..7686584`, in ascending order;
  - `ENSG00000141510.tsv.gz` (TP53), inclusive source positions
    `chr17:7686072..7687427`, in descending order.
  - `ENSG00000169129.tsv.gz`, inclusive `chr10:114306065..114306067`,
    preserving a real `REF=N` locus whose alternate set omits `A`;
  - `ENSG00000175727.tsv.gz`, inclusive `chr12:122093259..122093261`,
    preserving a real `REF=N` locus whose alternate set omits `T`;
  - `ENSG00000185974.tsv.gz`, inclusive `chr13:113673020..113723021`, whose
    source contains only the two endpoint loci and preserves the real
    50,000-base omission between them.
- Add a fixture README that identifies “Pangolin precomputed scores,” creators
  Nils Wagner and Aleksandr Neverov, Zenodo record/DOI, CC BY 4.0, archive
  checksum, the exact extraction ranges, the date of extraction, deterministic
  extraction/recompression commands, and that Pangopup selected, truncated,
  and recompressed the source while leaving header and row values unchanged.
  Record SHA-256 for every committed gzip member and its decompressed bytes;
  recompression uses a fixed no-name/no-timestamp gzip representation.
- Add the minimum reusable value types to `pangopup-core`: primary GRCh38
  contig, one-based genomic position, concrete DNA base, Ensembl gene ID, score
  magnitude in exact hundredths, relative genomic position in `-50..=50`, a
  `Grch38Snv` composed from the genomic scalars, and a four-value
  `PangolinScore`. Constructors validate their invariants and expose typed
  errors. `pangopup-build` owns the gene-qualified source record and the
  `REF=N` exception record. Defer the public lookup result, source provenance
  record, provider trait, HGVS, transcripts, proteins, gene metadata, and a
  general assembly framework until a runtime provider exists.
- Add streaming `.tsv.gz` ingestion to `pangopup-build`. Parsing must yield or
  visit one typed row/locus at a time and must not collect a whole source file or
  source directory merely to validate it. Builder-only gzip/TSV dependencies
  remain in this crate.
- Validate the exact eight-column header, filename-derived Ensembl gene ID,
  field count and field syntax, supported primary chromosome spelling,
  one-based position, concrete ordinary reference/alternate bases, distinct
  REF and ALT, gain range `0.00..=1.00`, loss range `-1.00..=0.00`, exact
  hundredth grid, and both relative positions in `-50..=50`.
- Validate each ordinary locus as exactly three adjacent rows with one shared
  chromosome/position/reference and exactly the other three alternate bases.
  Accept either consistently ascending or consistently descending positions in
  one file; reject a direction reversal or a locus that reappears after another
  locus. Report coordinate gaps as segment boundaries rather than rejecting
  them. Reject mixed chromosomes in one gene file.
- Normalize positive and negative textual zero to one numeric zero while
  preserving every nonzero score exactly. Convert source loss values to a loss
  magnitude only after validating their sign; rendering restores the minus
  sign for a nonzero loss.
- Preserve the archive's `REF=N` shape in a distinct build-only source-exception
  type and summary count. It is not a valid public genomic SNV and is never
  converted to the ordinary three-alternate record. Synthetic tests cover both
  observed three-alternate exception shapes (omit `A` and omit `T`).
- Add a `pangopup-build inspect <SOURCE_DIR>` binary. It discovers every direct
  regular `.tsv.gz` member, sorts by filename, and then validates each filename
  as exactly `ENSG###########.tsv.gz`; an invalid TSV member fails instead of
  disappearing during discovery. Ignore direct non-TSV files and nested
  directories, reject a source directory with zero TSV members, and reject a
  direct `.tsv.gz` symlink without following it.
- The command emits one canonical line per file in filename order, followed by
  one total line. `first` and `last` mean positions in source order; `gaps` is
  the number of adjacent locus transitions with omitted bases; `omitted_bases`
  is the sum of those missing genomic positions; and `segments = gaps + 1` for
  each nonempty file. The exact grammar is:

  ```text
  file gene=<ENSG> contig=<chr> direction=<ascending|descending> first=<u32> last=<u32> rows=<u64> loci=<u64> segments=<u64> gaps=<u64> omitted_bases=<u64> ambiguous_ref_loci=<u64> n_omit_a=<u64> n_omit_t=<u64>
  total genes=<u64> rows=<u64> loci=<u64> ascending=<u64> descending=<u64> segments=<u64> gaps=<u64> omitted_bases=<u64> ambiguous_ref_loci=<u64> n_omit_a=<u64> n_omit_t=<u64>
  ```

  All aggregate counts use `u64`. On the checked fixture the total is exactly
  6 genes, 6,342 rows, 2,114 gene-loci, 4 ascending files, 2 descending files,
  9 segments, 3 gap transitions, 50,002 omitted bases, 2 ambiguous-reference
  loci, one omit-A shape, and one omit-T shape.
- Errors are typed internally and rendered by the binary with the source member,
  one-based line number when applicable, and a precise reason. Any invalid
  member makes inspection nonzero and prevents a success total. Exit status is
  0 for valid input, 1 for source/I/O validation failure, and 2 for CLI usage.
- Add an executable `spec/source-inspect.md` contract for the valid fixture and
  checked `tests/fixtures/pangolin-precompute-malformed/` failure. Its sole
  `ENSG00000000003.tsv.gz` member repeats ALT `G` on line 4 of one `chr1:100 A`
  group; stderr is exactly
  `error: ENSG00000000003.tsv.gz:4: duplicate alternate G at chr1:100 A` and the
  exit status is 1. Update `Makefile` so `make spec` builds both CLI binaries
  needed by the specs.
- Before review, run the production parser once against an operator-supplied
  extracted source directory using `PANGOPUP_SOURCE_DIR`; this is retained
  implementation evidence, not part of an ordinary gate and never triggers a
  download. Its total must match 19,913 genes/files, 4,099,255,665 rows,
  1,366,418,555 gene-loci, 10,073 ascending and 9,840 descending files, 19,916
  segments, 3 gap transitions, 50,002 omitted bases, 30 `REF=N` loci, 9 omit-A
  shapes, and 21 omit-T shapes.
- Update `README.md` current-state/development-order text and
  `planning/frontier.md` after the behavior lands. Update `architecture/design.md`
  or `architecture/index.md` only if implementation reveals a durable contract
  not already recorded; do not restate code mechanically.
- Excluded: binary index encoding, mmap, lookup/provider implementation,
  benchmarks of candidate index layouts, full-corpus rebuilding, reference
  FASTA checks, asset download/install, model execution, HTTP, and caching.

## Success Checklist

- `pangopup-build inspect tests/fixtures/pangolin-precompute/` succeeds and
  prints six deterministic per-file lines plus the exact canonical total above;
  this is exercised through `spec/source-inspect.md` and `make spec`.
- The same command over a malformed fixture exits nonzero and identifies the
  member, line, and invariant violated; the executable spec covers one case and
  inside-out tests cover all remaining error families.
- Unit tests prove every value-type boundary, exact decimal-to-centi conversion,
  signed-zero normalization, signed loss restoration, and relative positions at
  both `-50` and `+50`.
- Parser tests cover the exact header, wrong field count, invalid filename/gene,
  invalid chromosome/base/position/decimal/range, REF=ALT, incomplete or
  duplicate alternate groups, split/reappearing loci, direction reversal,
  mixed chromosomes, empty/no-member input, ignored unrelated entries,
  rejected symlinks, accepted ascending and descending files, accepted gaps,
  and both valid `REF=N` exception shapes.
- A fixture exactness test proves all 6,342 committed rows parse and that the
  selected corpus contains each ordinary reference base, all three alternates
  for every ordinary locus, zero and nonzero gain/loss values, nonzero values at
  relative-position boundaries, both real `REF=N` shapes, all three real source
  gap transitions, and different TP53/WRAP53 scores for the same genomic allele
  at `chr17:7686072 G>T`.
- The non-gate complete-source run uses `PANGOPUP_SOURCE_DIR` and matches every
  count listed in Scope. Record the command, compact total, peak RSS, and source
  archive checksum in Implementation Evidence without recording the local
  source path.
- The ingestion API and inspection implementation are streaming by construction:
  no `Vec` or map of all rows/loci is returned or retained. A focused test uses
  a counting visitor/iterator consumer to prove records are delivered in source
  order; code review checks that directory aggregation stays proportional to
  member count and one in-flight locus.
- No full downloaded dataset, generated report, or machine-specific absolute
  path is committed. Ordinary gates require only the checked fixture.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

### Pin real gzip members as range excerpts

- Consideration: tests need realistic row spelling, source direction, overlap,
  and score boundaries without checking in whole gene files or depending on an
  operator download.
- Options: synthesize every valid row; check in complete source members; check in
  small real excerpts in the original gzip/member shape.
- Trade-offs: synthetic rows are easy to read but can encode our assumptions;
  full members are needlessly large; real excerpts are small and faithful but
  require explicit provenance and must state that they are truncated.
- Decision: pin the six exact real excerpts named in Scope, with CC BY
  attribution and extraction metadata. Keep malformed cases synthetic so each
  failure has one intentional defect.

### Parse exact decimal hundredths, never binary floats

- Consideration: source scores are decimal hundredths and future lookup must
  reproduce them exactly.
- Options: parse `f32`; parse a decimal library type; validate the narrow source
  grammar directly into integer hundredths.
- Trade-offs: `f32` introduces representation and equality ambiguity; a general
  decimal dependency is broader than the source domain; a small strict parser is
  exact and cheap but must explicitly handle signed zero and reject extra
  precision.
- Decision: parse directly into validated integer hundredths. Core stores gain
  and loss magnitudes as `0..=100`; the source adapter validates that nonzero
  gain is positive and nonzero loss is negative before conversion.

### Keep source exceptions below the public SNV boundary

- Consideration: the archive contains 30 `REF=N` loci, while a genomic SNV needs
  a concrete reference and alternate allele.
- Options: reject the files; guess a reference from GRCh38; broaden the public
  SNV type to permit `N`; preserve them only in a build-side exception record.
- Trade-offs: rejection loses source fidelity; guessing needs the later pinned
  FASTA and changes published identity; broadening makes invalid runtime requests
  representable; a separate exception path adds one branch but keeps both
  contracts truthful.
- Decision: parse and count the observed `N` groups in a build-only exception
  type. Do not expose them as ordinary core SNV score records.

### Expose source characterization through a separate builder binary

- Consideration: the source contract needs outside-in proof, while gzip/TSV
  dependencies must stay off the runtime lookup path.
- Options: unit tests only; add source inspection to the user-facing `pangopup`
  binary; add a `pangopup-build` administrative binary next to its library.
- Trade-offs: unit tests are not an observable workflow; the runtime CLI would
  acquire build-only concerns; a separate binary adds one small command surface
  but preserves crate boundaries and naturally grows into the future offline
  index builder.
- Decision: add `pangopup-build inspect`. It reports source facts only and does
  not write an index.

### Stream records and canonicalize only at the future writer boundary

- Consideration: source files arrive in both coordinate directions, and the full
  corpus is far too large to materialize.
- Options: collect and sort each file; require ascending input; emit validated
  source order with direction metadata and let the future writer reverse or
  place records as it writes.
- Trade-offs: collecting is simple but violates the memory goal; rejecting
  descending input rejects nearly half the archive; source-order streaming keeps
  memory bounded but means canonical output ordering belongs to the index writer.
- Decision: the ingestion API streams validated source order and reports one
  file direction. This ticket does not canonicalize or write an index.

## Dependencies

None.

## Notes

- The source archive is Zenodo record `15649338`; obtain the canonical DOI and
  license URL from that record for the fixture README. Do not copy local paths
  into tracked files.
- The source header is exactly:
  `chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos`.
- The selected excerpt totals were independently calculated from the verified
  archive. WRAP53 contributes 1,539 rows / 513 loci; TP53 contributes 4,068 rows
  / 1,356 loci; `ENSG00000010610` contributes 711 rows / 237 loci;
  `ENSG00000185974` contributes 6 rows / 2 loci; and the two three-locus
  `REF=N` excerpts contribute 9 rows each. WRAP53 and TP53 both cover all four
  ordinary reference bases. WRAP53 contains nonzero gain positions at `-50`
  and `+50`; TP53 contains nonzero gain and loss positions at both boundaries.
- The overlap oracle is intentionally gene-specific:
  `chr17:7686072 G>T` has `(gain=0.35, gain_pos=25, loss=0,
  loss_pos=-50)` for WRAP53 and all-zero/default positions for TP53. Both records
  must survive; inspection or tests must not deduplicate them by genomic allele.
- Except for the one named malformed spec fixture, tests create malformed
  `.tsv.gz` members in a temporary directory. Do not commit a large matrix of
  nearly identical malformed gzip files.
- The complete-corpus counts and source exceptions are retained in
  `planning/artifacts/2026-07-20-full-dataset-entropy.md`. They are evidence, not
  an input required by the gates.
- Run the exact final gates from the repository root:

  ```text
  make lint
  make test
  make spec
  ```

## Independent Ticket Review

Reviewer: `ticket_001_review`

Initial decision: changes required.

1. Discovery previously filtered invalid filenames before validation. Revised
   it to discover all direct `.tsv.gz` files, validate their names afterward,
   ignore only named unrelated shapes, reject empty input, and define symlink
   behavior.
2. The core/source record ownership was ambiguous. Defined exact core scalars,
   `Grch38Snv`, and `PangolinScore`; kept gene-qualified source rows and `REF=N`
   exceptions in the builder; deferred lookup results, provenance, and traits.
3. Valid rare shapes were synthetic only. Added attributed real excerpts for
   both `REF=N` alternate sets and every real coordinate-gap transition, with
   exact updated totals.
4. Miniature proof alone did not establish full-corpus parity. Added one
   non-gate `PANGOPUP_SOURCE_DIR` run with exact retained totals, `u64` counters,
   peak-RSS evidence, and no automatic download or recorded local path.
5. CLI output and failures were underspecified. Defined stdout grammar, terms,
   exit statuses, the checked malformed fixture, and its exact stderr.
6. Fixture fidelity was not reproducible. Required deterministic recompression,
   compressed/decompressed SHA-256 values, exact commands, and accurate
   selected/truncated/recompressed wording.

Re-review decision: approved. The reviewer independently verified the revised
fixture arithmetic against the downloaded source: 6 genes, 6,342 rows, 2,114
gene-loci, 4 ascending and 2 descending files, 9 segments, 3 gap transitions,
50,002 omitted bases, and the two named `REF=N` shapes. No material findings
remain. A non-material wording ambiguity about symlinks was made explicit before
marking the ticket ready.

## Implementation Evidence

Developer: pending

Record focused tests, measurements, generated artifact identities, and any
scope-relevant deviation, then set status to `review`. The developer cannot be
either reviewer.

## Adversarial Code Review

Reviewer: pending

Record diff/test findings and their disposition before completion. The reviewer
is read-only and cannot be the ticket reviewer or developer. Material fixes are
returned to this reviewer. The ticket may become `complete` and enter final
gates only after the reviewer records approval.
