# 010 — Select the compact GRCh38 runtime reference format

Status: ready
Accepted contract identity: `sha256:0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee`
Base revision: `36fee3b5d6551f0dc24c5d42d57986c283c98c28`

## Why

Pangopup cannot run model fallback until a long-lived Rust process can read an
exact GRCh38 sequence window without parsing FASTA at request time. The exact
RefSeq assembly, primary accessions, normalized sequence-set digest, and model
contexts are already pinned, but no runtime reference representation has been
selected.

This is the next critical-path slice. It selects one measured reference payload
encoding and records that decision. It deliberately stops before production
format hardening: combining an unknown format winner with a production bundle,
reader, CLI, and full 25-contig build was rejected during pre-ticket review as
too conditional for one independently reviewable ticket.

The observable outcome is a deterministic, independently checked benchmark
report and accepted ADR naming the one payload encoding that the next ticket
may harden. No runtime reference provider is claimed by this ticket.

## Current facts and provenance

- Base `main` and `origin/main` are the same clean revision named above.
- Ticket 009 pinned profile `pangolin-1.0.2-5cf94b8-grch38-v1`, the twelve
  checkpoints, RefSeq GRCh38.p14, and the exact upstream behavioral corpus.
- The full source is NCBI RefSeq assembly `GCF_000001405.40`, gzip size
  `972898531`, SHA-256
  `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`.
  It already exists read-only at:
  `/home/ian/foss/uta/ncbi-data/genomes/refseq/vertebrate_mammalian/Homo_sapiens/all_assembly_versions/GCF_000001405.40_GRCh38.p14/GCF_000001405.40_GRCh38.p14_genomic.fna.gz`.
  The adjacent assembly report is `80454` bytes with SHA-256
  `64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38`.
  Neither full input is read by this ticket.
- The exact representative input is the retained six-contig plain FASTA at
  `/home/ian/workspace/data/pangopup-compat-inputs/refseq-grch38p14-compat-six-contigs.fa`:
  `671294255` bytes, SHA-256
  `81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb`.
  Its contigs are `chr3`, `chr10`, `chr12`, `chr13`, `chr17`, and `chrM`.
- The independent expected windows are the literal `context.start_1based`,
  `context.bases`, and `context.sha256` fields of model cases M01–M14 in
  `tests/fixtures/pangolin-compat-v1/cases.jsonl`, whose complete member
  SHA-256 is
  `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`.
  The corpus manifest is `5337` bytes with SHA-256
  `fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b`.
  A candidate writer or reader must not generate its own expected bytes.
- The canonical complete 25-primary-sequence logical identity, retained before
  this ticket, is `3088286401` bases and SHA-256
  `2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`.
  It belongs to later production qualification, not this benchmark.
- `pangopup-build::production` already owns bounded streaming FASTA/gzip
  parsing, uppercase normalization, the 25 accession map, and canonical
  sequence hashing. Ticket 010 may share fixed symbol/accession constants, but
  its independent oracle must not call the candidate decoder or infer expected
  bytes from a candidate.
- Pre-ticket independent review required this selection ticket to be separate
  from production promotion, to close the candidate byte layouts and ranking
  arithmetic, and to keep full-source work out of the selection run.
- Two orphaned Ticket 007 subprocess-test helpers (PIDs observed as `2824225`
  and `2824319` at ticket authorship) are pre-existing environment state. They
  are unrelated to Ticket 010 and must not be killed, adopted, or used as
  evidence by this work.

## Scope

### Included

- Add benchmark-only candidate codec/readers under `pangopup-index` and the
  exact authenticated candidate-preparation adapter under `pangopup-build`.
  No candidate codec becomes a production runtime reader in this ticket.
- Add the maintenance CLI grammar:

  ```text
  pangopup-build reference-candidates prepare \
    --source <EXACT_SIX_CONTIG_FASTA> \
    --corpus <PANGOLIN_COMPAT_CORPUS_DIR> \
    --output <ABSENT_CANDIDATE_DIR>

  pangopup-build reference-candidates inspect \
    --candidates <CANDIDATE_DIR> \
    --corpus <PANGOLIN_COMPAT_CORPUS_DIR>
  ```

  Flags appear exactly once, unknown/duplicate/missing flags are usage errors,
  and every invocation emits exactly one JSON line. `prepare` accepts only the
  exact six-contig and corpus identities above in non-test code, publishes to
  an absent directory, and never downloads. `inspect` accepts either that
  profile or the closed checked miniature fixture profile.
- Add a custom-harness benchmark named `reference_formats` in
  `pangopup-index`. It consumes the prepared candidate directory and exact
  corpus through explicit environment paths, writes one canonical JSON report
  to an explicit absent output path, and never discovers a home directory.
- Add a pure selection evaluator covered by unit tests. The evaluator, not the
  implementation agent, applies the ranking contract below.
- Retain a small checked candidate fixture and an executable inspect spec.
- Record exact identities, method, raw round summaries, resource limits,
  selected candidate, limitations, and commands in
  `planning/artifacts/010-reference-format-selection.md` and a new accepted
  format-selection ADR.
- Update `README.md`, `AGENTS.md`, `architecture/README.md`,
  `architecture/design.md`, `architecture/runtime-data.md`,
  `architecture/delivery.md`, `planning/frontier.md`, `planning/faq.md`, and
  `spec/reference-candidates.md`. Correct `architecture/design.md`'s stale
  statement that remote sync is unimplemented, without broadening product
  scope.

### Excluded

- A production reference manifest, bundle, codec, reader, provider trait, or
  user-facing reference slice command.
- Full 25-contig parsing, production candidate generation, verification,
  transport, XDG install/sync, GitHub release, or any public/external mutation.
- Downloading or copying the already preserved full FASTA or assembly report.
- GENCODE/GTF/gffutils/SQLite parsing, mask format/order, or gene-ID changes.
- Copying or converting Pangolin checkpoints; choosing a tensor runtime;
  model execution, inference parity, lookup routing, or result caching.
- HTTP, Docker, service lifecycle, accelerators, quantization, and release
  hardening.
- Python, PyTorch, Pangolin execution or compatibility recapture.
- Rebuilding, hashing, verifying, or querying the 15 GB production SNV index.

## Candidate container and encodings

All three benchmark files use the same closed little-endian container so the
comparison changes only payload encoding.

### Candidate-set envelope

The prepared output is a flat directory containing exactly four regular files,
with no symlinks, subdirectories, or extra entries:

```text
manifest.json
ascii8.pgr
iupac4.pgr
acgt2-rle-v1.pgr
```

`manifest.json` is at most 16384 bytes and is the RFC 8785 serialization, with
no trailing newline, of this closed schema (all object fields are required and
unknown fields are rejected):

```text
{
  "schema": "pangopup-reference-candidates-v1",
  "profile": string,
  "source": {"bytes": u64, "sha256": 64-lowercase-hex},
  "corpus": {
    "schema": "pangopup-compat-v1",
    "profile": string,
    "manifest_bytes": u64,
    "manifest_sha256": 64-lowercase-hex,
    "cases_bytes": u64,
    "cases_sha256": 64-lowercase-hex
  },
  "container": {
    "schema": "pgrben01-v1",
    "page_bytes": 4096,
    "contigs": [u8]
  },
  "members": [
    {"codec":"ascii8", "filename":"ascii8.pgr", "bytes":u64,
     "sha256":64-lowercase-hex},
    {"codec":"iupac4", "filename":"iupac4.pgr", "bytes":u64,
     "sha256":64-lowercase-hex},
    {"codec":"acgt2-rle-v1", "filename":"acgt2-rle-v1.pgr", "bytes":u64,
     "sha256":64-lowercase-hex}
  ]
}
```

The `members` order shown is mandatory. The candidate-set identity is the
lowercase SHA-256 of the exact canonical `manifest.json` bytes; the manifest
does not contain that identity and therefore has no self-reference.

The real profile is
`refseq-grch38p14-compat-six-contigs-v1`. Its source size/hash and corpus
schema/profile are the pinned values in Current facts. Its corpus manifest is
authenticated by its exact size and SHA-256; the manifest in turn must name
the exact `cases.jsonl` size/hash above. `prepare` first calls the existing
strict compatibility inspector and then binds those identities; it does not
trust a caller-supplied manifest field alone.

The miniature profile is `pangopup-reference-candidates-mini-v1`. Its source,
corpus manifest, cases, four output files, and canonical manifest are checked
fixtures whose exact sizes and SHA-256 values are compiled into the inspector.
Only test support may prepare that profile; the shipped `inspect` command may
inspect it so the executable spec remains offline and small. Both profiles
must match their complete compiled registry entry. A profile selected merely
by filename, a cross-profile combination, a changed member, or a self-consistent
but unregistered manifest fails. The fixture source and literal expectation
file are independently authored; neither is decoded from candidate output.

### Common container

- Bytes `0..8`: ASCII magic `PGRBEN01`.
- `8..10`: `u16` version `1`.
- `10`: codec `1=ascii8`, `2=iupac4`, `3=acgt2-rle-v1`.
- `11`: `u8` contig count.
- `12..16`: zero reserved bytes.
- `16..24`: `u64` exact file length.
- `24..32`: `u64` directory offset, exactly `64`.
- `32..40`: `u64` directory length, exactly `contig_count * 48`.
- `40..48`: `u64` payload offset, exactly `4096`.
- `48..56`: `u64` payload length, exactly `file_length - 4096`.
- `56..64`: zero reserved bytes.
- `contig_code` is exactly `pangopup_core::Grch38Contig::code()`:
  `chr1..chr22 = 1..22`, `chrX = 23`, `chrY = 24`, and `chrM = 25`. The real
  profile has exactly the ordered codes `[3,10,12,13,17,25]`.
- Each 48-byte directory entry is sorted by that contig code and contains:
  `u8 contig_code`, seven zero reserved bytes, `u64 base_length`, `u64 data_offset`,
  `u64 data_length`, `u64 auxiliary_offset`, and `u64 auxiliary_count`.
- Bytes between the directory end and byte 4096 are zero. All offsets are
  absolute. Sections are nonoverlapping and contained in the exact file.
  `directory_offset + directory_length <= 4096`. Contig data appear in
  directory order and cover the payload through exact file length with no
  leading, inter-contig, or trailing bytes except the explicit two-bit
  auxiliary alignment below.

The common IUPAC code map is:

```text
A=0 C=1 G=2 T=3 R=4 Y=5 S=6 W=7 K=8 M=9 B=10 D=11 H=12 V=13 N=14
```

Code `15` is reserved.

### `ascii8`

- One uppercase ASCII byte per base.
- `data_length == base_length`; auxiliary offset/count are zero.

### `iupac4`

- Two codes per byte. The earlier genomic base occupies the low nibble and the
  later base the high nibble.
- `data_length == (base_length + 1) / 2` with checked arithmetic.
- For odd contig length, the unused final high nibble is reserved code `15`;
  code `15` anywhere else is invalid. Auxiliary offset/count are zero.

### `acgt2-rle-v1`

- `A/C/G/T = 0/1/2/3`, four bases per byte in genomic order from least- to
  most-significant two-bit pair. Unused high pairs in the last byte are zero.
- `data_length == (base_length + 3) / 4` with checked arithmetic.
- Every non-ACGT base has dense placeholder `A` and is restored by an exact
  ambiguity run. Consecutive identical ambiguity symbols form one maximal run.
- The per-contig auxiliary table begins at the next eight-byte boundary after
  its dense bytes when `auxiliary_count > 0`. Alignment padding is zero. Each
  16-byte run is
  `u32 zero_based_start`, `u32 length`, `u8 IUPAC code 4..14`, followed by
  seven zero reserved bytes. Runs are nonempty, sorted, nonoverlapping,
  in-bounds, and adjacent equal-code runs are invalid rather than silently
  accepted.
- `auxiliary_count * 16` is the exact auxiliary byte length. If the count is
  zero, `auxiliary_offset` is exactly zero and the next contig begins
  immediately after dense data. Otherwise `auxiliary_offset` is the first
  eight-byte-aligned address at or after dense end, the next contig begins
  immediately after auxiliary end, and the only permitted gap is that zeroed
  alignment. `zero_based_start`, `length`, and their checked sum must fit
  `u32`; a source contig or run that cannot be represented is rejected rather
  than truncated.
- Window copy decodes dense bytes through a fixed byte-to-four-ASCII lookup,
  binary-searches the first run whose end exceeds the requested start, and
  overlays only intersecting runs.

All readers copy into caller-owned storage. After sufficient capacity is
established, the measured operation performs no heap allocation. Benchmark
files provide only canonical `chr` contigs; alias policy belongs to Ticket 011.

For every codec, the first contig's `data_offset` is exactly 4096. Each later
offset follows the preceding codec-specific end rule above, and exact file
length is the final contig end. ASCII has `data_length == base_length` and
zero auxiliary fields. ASCII and four-bit therefore have no payload gaps.

## Benchmark and selection decision

### Maintenance CLI contract

Success is exit `0`; operational failure is exit `1`; grammar/usage failure is
exit `2`. Standard output is exactly one RFC 8785 JSON object plus LF and
standard error is empty. Success objects are exactly:

```text
{"ok":true,"command":"reference-candidates.prepare",
 "profile":string,"candidate_set_sha256":hex,
 "members":[{"codec":string,"bytes":u64,"sha256":hex} x3]}

{"ok":true,"command":"reference-candidates.inspect",
 "profile":string,"candidate_set_sha256":hex,
 "source_sha256":hex,"corpus_manifest_sha256":hex,
 "contexts_verified":u64,
 "members":[{"codec":string,"bytes":u64,"sha256":hex} x3]}
```

Member order is always `ascii8`, `iupac4`, `acgt2-rle-v1`. Errors are exactly
`{"ok":false,"command":string,"error":{"code":string,"message":string}}`
plus LF. Closed error codes are `usage`, `path`, `already_exists`,
`unsupported_profile`, `source_identity`, `corpus_identity`, `candidate_set`,
`candidate_member`, `container`, `oracle`, `bounds`, `resource`, and `io`.
Messages are stable short English summaries: they contain no source bytes,
absolute paths, OS error text, backtraces, or partial hash. An error before a
subcommand is identified uses command `reference-candidates`; otherwise the
two command strings above are used.

`prepare` creates a private sibling staging directory, writes and syncs all
members and the manifest, syncs the directory, and performs a no-replace
rename to the caller's absent output. On error it removes unpublished staging;
it never changes an existing output. `inspect` requires the complete closed
directory, hashes and validates every member, validates all containers and
literal contexts, and emits no files.

### Benchmark process contract

The `reference_formats` harness requires exactly these environment variables,
each once in the inherited environment and each an absolute path:

```text
PANGOPUP_REFERENCE_CANDIDATES  prepared closed candidate directory
PANGOPUP_REFERENCE_CORPUS      exact compatibility corpus directory
PANGOPUP_REFERENCE_REPORT      absent output file
```

Missing, non-Unicode, relative, aliased-same-file, or unexpected profile paths
fail before timing. The report is privately staged and no-replace renamed; an
existing report is never replaced. Success writes one RFC 8785 JSON object
with no trailing newline. Failure leaves no report and exits nonzero with one
sanitized error line.

The closed report schema is:

```text
{
 "schema":"pangopup-reference-format-benchmark-v1",
 "contract_sha256":hex,
 "candidate_set_sha256":hex,
 "source":{"bytes":u64,"sha256":hex},
 "corpus":{"manifest_bytes":u64,"manifest_sha256":hex,
           "cases_bytes":u64,"cases_sha256":hex},
 "environment":{"rustc":string,"target":string,"os":string,"kernel":string,
                "cpu":string,"logical_cpus":u64,"power":string,
                "affinity":string},
 "method":{"page_bytes":4096,"rounds":5,"warmups_per_round":20,
           "operations_per_round":10000,"quantile":"nearest-rank",
           "candidate_orders":[[string;3];5]},
 "candidates":[{
   "codec":string,"member_bytes":u64,"member_sha256":hex,
   "open_ns":[u64;5],"round_p50_ns":[u64;5],"round_p95_ns":[u64;5],
   "headline_p50_ns":u64,"headline_p95_ns":u64,
   "allocation_calls_per_copy":u64,"allocation_bytes_per_copy":u64,
   "logical_bases":u64,"unique_pages":[u64],"unique_page_count":u64,
   "zstd_bytes":u64
 } x3],
 "process":{"maximum_rss_bytes":u64,"minor_faults":u64,"major_faults":u64},
 "selection":{"status":"selected"|"proposed","codec":string|null,
              "reason":"speed"|"pages"|"file_bytes"|"zstd_bytes"|
                       "invalid_evidence"|"exact_tie"}
}
```

Candidate and candidate-order strings use the three exact codec names. Arrays
and candidates appear in the fixed order defined by the operation stream.
`unique_pages` is sorted ascending with no duplicates and its length equals
`unique_page_count`. All times are integer nanoseconds; sizes are bytes; page
numbers are zero-based. Open cost is measured once immediately before each
round's block with the same mmap/open validation path. Environment strings are
retained facts rather than selection inputs. The artifact separately records
candidate preparation wall/user/system time, maximum RSS, scratch bytes,
minor/major faults, host power/affinity controls, and the exact report hash.

### Fixed operation stream

- Validate every candidate against all literal M01–M14 context bytes and
  hashes before any timing. Any mismatch makes the candidate ineligible and
  fails the run.
- One operation copies the complete literal context window into a preallocated
  buffer and hashes/black-boxes the copied bytes outside the timed interval.
- Each candidate has one long-lived mmap reader per round.
- Each of five rounds performs 20 untimed warmups followed by exactly 10000
  retained operations in M01…M14 cyclic order (`operation_index % 14`). Thus
  the first six cases appear 715 times and the remaining eight 714 times.
- Candidate block order by round is fixed:
  `ascii8/iupac4/acgt2`, `iupac4/acgt2/ascii8`,
  `acgt2/ascii8/iupac4`, `ascii8/acgt2/iupac4`, and
  `acgt2/iupac4/ascii8`.
- Timing storage and buffers are allocated and touched before counters reset.
  An empty-operation control must report zero allocation calls and bytes.
- Each round reports nearest-rank p50 and p95 from sorted retained nanoseconds:
  index `ceil(p * n) - 1`. The candidate headline value is the third sorted
  value of its five round quantiles.

### Metrics and deterministic winner

1. A unique speed winner exists only when, for both headline p50 and p95, its
   integer nanoseconds satisfy `winner * 100 <= opponent * 95` for every other
   exact candidate. Use checked `u128` multiplication.
2. If no speed winner exists, choose the candidate with the fewest unique
   4096-byte mapped file pages in the deterministic logical read trace for
   open plus one each of the 14 distinct operations. A trace maps every
   half-open byte range `[start,end)` actually required by the specified
   decoder to absolute page numbers `start/4096 .. (end-1)/4096`, then unions
   and sorts them. Open contributes exactly `[0,64)` and
   `[64,64 + contig_count*48)`; it does not contribute header padding.
   ASCII contributes only intersecting data bytes; four-bit contributes the
   minimal intersecting packed-byte range. Two-bit contributes its minimal
   dense-byte range plus every complete 16-byte run record examined by this
   exact lower-bound search: initialize `low=0, high=count`, inspect
   `mid=low+(high-low)/2`, set `high=mid` when checked run end exceeds window
   start and otherwise set `low=mid+1`; from the resulting index inspect
   consecutive records until run start is at or beyond window end. Untouched
   alignment and padding never count. Empty ranges contribute no page.
3. If still tied, choose the smallest complete candidate file in bytes.
4. If still tied, choose the smallest deterministic Zstandard frame over the
   complete candidate file using `zstd 0.13.3`, `zstd-safe 7.2.4`, bundled
   libzstd `1.5.7`, level 9, checksum and content size enabled, dictionary ID
   disabled, long-distance matching disabled, zero workers, and the exact
   pledged input size.
5. If still exactly tied, or if arithmetic/resource/oracle evidence is invalid,
   do not select. Return the ticket to `proposed` for a new reviewed decision.

Record open cost, five p50/p95 pairs, allocation calls/bytes per copy, logical
bytes, unique pages, complete bytes, Zstandard bytes, process maximum RSS,
minor/major faults, and candidate preparation resources. These are retained
observations, not machine-dependent test thresholds. Make no cold-I/O claim.

## Red and discriminating controls

- A manually asserted miniature source covers all 15 uppercase IUPAC symbols,
  lowercase normalization, odd packing lengths, two-bit padding, adjacent and
  separated ambiguity runs, and windows crossing nibble, byte, run, contig,
  and 4096-byte boundaries. Its expected decoded sequences are literal test
  constants, not candidate output.
- Mutate a source literal while keeping candidate files self-consistent: the
  committed independent expectation must fail.
- Mutate candidate header, codec, reserved/padding bits, directory ordering,
  offset/length arithmetic, IUPAC code, two-bit run bounds/order/coalescing,
  file truncation/trailing bytes, candidate-set member hash, and corpus hash;
  inspect must fail with a typed sanitized error.
- Zero length, zero start, checked-end overflow, unknown contig, and left/right
  out-of-range windows fail without modifying the destination buffer.
- Read auditing proves open consumes only the bounded header/directory and does
  not traverse payload; a window touches only its declared page set. Malformed
  bytes in a touched encoded symbol/run fail. Valid-code bit flips are detected
  by corpus/member integrity during `inspect`, not falsely claimed as
  per-window checksums.
- The miniature control contains literal, manually calculated sorted page
  arrays for open and every window under all three codecs, plus their union
  counts. Tests compare the reader's logical trace to those constants; they do
  not derive expected pages through the production trace implementation.
- Two miniature preparations produce byte-identical candidate sets. The large
  six-contig preparation runs once only after the exact harness is reviewed.

## Success checklist

- The exact six-contig source and compatibility corpus are authenticated before
  candidate generation; no filename alone is trusted.
- All three candidates decode all 14 contexts and the independent miniature
  oracle exactly before timing.
- Candidate preparation and inspection use bounded streaming input and do not
  hold the 671 MB source or decoded candidate in heap. Retain peak RSS, scratch,
  and output sizes.
- The evaluator produces exactly one winner under the closed ranking rule, and
  the accepted ADR names only that payload encoding for Ticket 011.
- `cargo test --locked -p pangopup-index reference_candidate` passes.
- `cargo test --locked -p pangopup-build --test reference_candidates` passes.
- `mustmatch test spec/reference-candidates.md` passes on the miniature fixture.
- `make lint`, `make test`, and `make spec` pass after independent code-review
  acceptance.
- No normal gate or CI job reads large local inputs or runs the retained
  benchmark.
- The immutable public SNV release/profile remains untouched. If the repository
  builder-source fingerprint changes, regenerate only the tiny checked SNV
  regression fixture and prove `scores.pgi`, request semantics, and expected
  output after removing provenance remain byte-identical.

## Decisions

1. **Split selection from production.** The alternatives were one conditional
   large ticket, preselecting two-bit without evidence, or a selection ticket
   followed by winner-specific production hardening. Select the last option so
   each independent review has a fixed contract.
2. **Compare exactly three representations.** ASCII is the direct-copy speed
   baseline, four-bit is the simple exact-IUPAC midpoint, and two-bit plus runs
   is the compact baseline. Compressed blocks are excluded because request-time
   decompression conflicts with the established mmap direction.
3. **Use a material speed dominance rule.** A five-percent win on both p50 and
   p95 takes priority; otherwise pages, installed size, then download encoding
   decide. This honors speed first without treating measurement noise as a
   product decision.
4. **Keep the oracle independent.** Committed literal model contexts and manual
   IUPAC controls define expected bytes. The new writer and reader never define
   their own expected result.
5. **Do not use full production input in Ticket 010.** The representative input
   already contains exact real contexts at useful scale. The one full build is
   reserved for Ticket 011 after the format is known and hardened.

## Dependencies

- Ticket 009 is complete.
- Exact six-contig source and compatibility corpus exist locally with pinned
  identities.
- No external or unresolved dependency blocks implementation.

## Work ownership

- Coordinator-authored change before readiness: this ticket only.
- Pre-existing user changes: none in the Git worktree.
- Pre-existing unrelated process state: the two orphaned Ticket 007 helpers
  named above; preserve them.
- Implementer changes: pending after independent design acceptance.
- Generated artifacts: miniature checked fixture plus large candidate/output
  paths named in the long-job checkpoint; pending.
- Concurrent unrelated work: none known.
- Coordinator: `/root`.
- Independent design reviewer: `/root/ticket_009_code_review`, acting in a new
  role on Ticket 010 and not eligible to implement or code-review Ticket 010.
- Developer: pending, must be a different subagent.
- Adversarial code reviewer: pending, must differ from coordinator, design
  reviewer, and developer.

## Long-running jobs

No job may begin before candidate code, oracle controls, measurement method,
selection evaluator, and focused miniature tests receive an initial read-only
adversarial review.

The later single retained run uses:

```text
input:  /home/ian/workspace/data/pangopup-compat-inputs/refseq-grch38p14-compat-six-contigs.fa
output: /home/ian/workspace/data/pangopup-reference-format-010/<CONTRACT_ID>/
logs:   the same output directory; compact evidence is copied into
        planning/artifacts/010-reference-format-selection.md
```

Before launch the coordinator records exact contract/diff, release binary,
source/corpus, candidate-container, oracle, evaluator, Zstandard, command,
output, process/session, free-space, progress, completion, failure, cleanup,
and cancellation identities. Output must be absent. Publication uses private
sibling staging and no-replace rename. Failure is not automatically retried;
inspect the cause first. An unchanged successful benchmark is never rerun. A
causal code, source, corpus, comparator, container, or selection change marks
the old output obsolete and requires the same reviewers before any replacement
run.

## Coordinator Authorship

Coordinator: `/root`

This proposed contract incorporates the pre-ticket independent rejection:
selection and production are split; layouts and ranking are closed; oracle and
large-input boundaries are explicit; and existing full inputs are preserved.

## Independent Ticket Review

Reviewer: `/root/ticket_009_code_review` — accepted

The reviewer is read-only. Material corrections return to the coordinator and
then this same reviewer. Only an accepted exact contract may become `ready`.

The reviewer rejected contract
`ac668a900a6bbc319c3974a686177a8e997920ad13fb7ec2cb380a46832b7c3e`.
The coordinator accepted all four findings: the candidate-set envelope was
undefined; some packed-length/section formulas and contig codes were implicit;
the CLI and benchmark report were not closed interfaces; and page accounting
did not define a deterministic logical read trace. This revision defines all
four. The reviewer agreed that the selection/production split, independent
oracle, and avoidance of needless full-source work were sound, and found no
separate major future issue.

The same reviewer then independently verified exact file SHA-256
`8640317cae7779d62d0567f6ec47dd488e46c183fda1b705f016428ade625a01`,
recomputed contract SHA-256
`0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee`,
confirmed base and `origin/main`, and accepted the corrected contract without
edits. All four findings are resolved. The reviewer found the contract bounded,
byte-exact, independently testable, and free of needless full-data work. No
remaining correction or separate major issue was found.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

The reviewer must first approve the candidate code, oracle, harness, evaluator,
and miniature evidence before the large retained run. The same reviewer then
reviews the run, selected ADR, final docs, and exact diff before completion and
answers whether a major separate issue was discovered.

## External Effect Evidence

Coordinator: not applicable

Ticket 010 performs no publication, release, upload, deployment, paid job, or
live-system mutation. The one retained local benchmark uses existing read-only
inputs and writes only to the named workspace data directory after review.

## Coordinator Final Check

Coordinator: pending

The coordinator will map every acceptance clause to evidence, audit the entire
changed-file set, run `make lint`, `make test`, and `make spec`, perform the
stale current/future documentation scan, and commit/push only after independent
code-review acceptance.

## Acceptance trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Exact reviewed contract | Contract SHA-256 `0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee`; independent acceptance above | Pass |
| Independent literal exactness | M01–M14 plus manual IUPAC controls | Pending |
| Closed candidates and corruption controls | Focused index/build suites | Pending |
| Deterministic selection | Retained report plus accepted ADR | Pending |
| Bounded resources and one retained run | Long-job checkpoint and artifact | Pending |
| Full repository gates | `make lint`; `make test`; `make spec` | Pending |
| Independent code review | Actual diff and evidence | Pending |

## Evidence and artifacts

- Base revision: `36fee3b5d6551f0dc24c5d42d57986c283c98c28`.
- Contract identity, reviewed-ready commit, implementation diff, candidate-set
  identity, binary/source/corpus/evaluator identities, role assignments,
  commands, results, limitations, and final review verdict are pending.
- Durable evidence target:
  `planning/artifacts/010-reference-format-selection.md`.
