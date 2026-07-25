# Ticket 011 — Production reference evidence

## Contract

Ticket 011 hardens only ADR 0009's selected `acgt2-rle-v1` encoding. The
production `PGRREF01` bundle, typed provider, authenticated source builder,
private certification, and maintenance CLI are implemented independently from
the benchmark format. Model, GENCODE mask, transport/install, release, HTTP,
and Docker remain outside this artifact.

## Full-data-free implementation evidence

The registered `pangopup-reference-mini-v1` profile has 25 synthetic sequences,
159 bases, every IUPAC symbol, edge windows, ambiguity transitions, and both
plain and deterministic single-member gzip source forms. Normal tests prove:

- exact provider copies, strict aliases/provenance, unchanged destination on
  bounds errors, concurrent reads, and bounded dense-page access;
- identical plain/gzip `reference.pgr` with intentionally source-bound
  manifest identities;
- structural header/padding/run rejection and the honest integrity boundary in
  which a valid dense substitution passes cheap open but fails private
  certification;
- zero allocations for a warmed copy, reader-open peak heap below 2 MiB, and
  miniature builder peak heap below 16 MiB;
- the independent production offset calculation: M01–M14 have per-case page
  sum 20 and unique pages 31748, 109204, 119053, 119054, 133714, 133715,
  152494, and 152495;
- canonical JSON maintenance behavior through executable specification.
- no-follow held-input authentication before decompression, closed decoded and
  record/accession ceilings, concurrent no-replace publication, and durable
  parent-sync rollback;
- an identity-bound qualification harness that reports and enforces all
  latency/allocation/heap/size/page/logical thresholds and records actual
  binaries, sources, members, resource logs, pinned Rust-Zstandard size, and
  host evidence.

Focused developer commands and results are recorded in the completed Ticket
011 history in Git.

## Retained production build and failed first qualification

The reviewer authorized contract
`1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01`.
Its one production build succeeded and is preserved unchanged under
`/home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/`.
It certified 25 sequences, 3,088,286,401 bases, all 14 contexts, and logical
sequence SHA-256
`2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`.
The canonical manifest/bundle ID is
`7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
`reference.pgr` is 772,091,760 bytes with SHA-256
`cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`;
builder peak outstanding Rust heap was 1,201,871 bytes.
The build executable, bundle, manifest, heap/resource logs, stdout, and command
remain retained evidence and must not be rebuilt merely because the separate
reader failed.

The qualification executable then exited 1 immediately with
`reference benchmark failed: corpus case`. It did not run the timing workload,
Zstandard measurement, or threshold evaluation, and the empty report is not
qualification evidence. The harness had first authenticated the exact
220,071-byte corpus and SHA-256
`2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`;
its narrow nested serde
projections then incorrectly rejected unrelated fields present in every real M
case. The repaired projection accepts unrelated fields but still requires and
types `id`, `input.contig`, `context.start_1based`, and `context.bases`. A
dedicated binary test now reads the exact pinned corpus and proves all 14
contexts, with separate missing/wrong-type controls.

## Replacement qualification implementation

Do not rerun the old benchmark identity and do not repeat the successful full
build. The preserved immutable build is sufficient technical evidence to
reuse: its exact old builder executable/source identity agrees with the bundle
manifest, the source/report identities are fixed, private certification
succeeded, and the heap/resource logs remain available.

However, the current v1 `qualification-input.json` does **not** honestly meet
the stronger preferred claim that a replacement contract pre-binds the exact
old build evidence. It pre-binds the old builder executable/source plus
source/report/corpus/workload and the benchmark executable/harness, while the
harness hashes the actual bundle members and build logs only into its output
report. It would technically accept the preserved build, but its new contract
ID would not itself commit to the old bundle ID, manifest/member hashes, heap
report, resource log, build stdout, and old build-contract ID.

No replacement run may start under v1. The accepted v2 no-rebuild path adds
the exact retained identities, keeps the old builder source and bundle checks
unchanged, binds the repaired executable, harness, and complete compiled source
inventory, and produces a new contract ID from its canonical input. The
implementation opens the prior root, input, corpus, and output parent
component-by-component without symlinks. It authenticates and parses the same
held bytes, maps the held `reference.pgr` descriptor, duplicates that descriptor
for Zstandard measurement, then checks device, inode, size, and full SHA-256 on
every held file after measurement.
Each bounded small evidence file is read into one authenticated owned buffer;
all semantic checks reuse that buffer. Its held descriptor is read again only
for the contract's final full-hash mutation check.

The v2 result is a small, private stage containing canonical command, resource,
report or failure receipts and copies of the replacement executable/source.
Every member is synced and sealed read-only before atomic no-replace
publication. The successful v1 bundle remains only in its original root; v2
never copies or rewrites the 772 MB member. Full-data-free tests exercise path
substitution, same-inode mutation, closed JSON nesting, exact member/mode sets,
concurrent no-replace, and durable failure preservation.
Every error after private-stage creation now reaches one failure funnel that
repairs the exact member set, writes closed receipts, seals modes, and publishes
without replacement. Injected controls cover initial retention, revalidation,
resource/report work, stage validation, success replacement, and failure
publication exhaustion; the last case proves the exact preserved stage path is
reported.

Publication validation authenticates the success report itself: it opens the
report relative to the held stage descriptor, parses the same authenticated
buffer as closed canonical JSON, and requires byte-for-byte equality with the
accepted in-memory report before success can be renamed into place. Changed,
truncated, noncanonical, duplicate-key, and unknown-field reports cannot
publish a success root.

Failure repair is likewise descriptor-relative. It inventories the held stage
and held child/candidate descriptors, binds directory device/inode identities,
and removes entries only with no-follow `*at` operations. It never enumerates
or deletes through mutable `stage.path`. Before rename or preserved-stage
reporting, it finds the held stage's actual current name relative to the held
parent descriptor. Full-data-free concurrent rename/substitution controls prove
that a tree substituted at the stale pathname is untouched and that both
published and preserved failure paths name the real held stage.

The final retained-success section below records query
quantiles/pages/allocations, open heap/dense reads, benchmark RSS/faults,
installed size, and measurement-only pinned-Zstandard size.

There was no further reference build launch. Each v2 candidate below ran only
once under its recorded independent review and coordinator authorization; none
may be rerun.

## First v2 qualification failure and diagnostic remediation

The reviewed v2 input contract
`9f22e22e41972db56fbca6d8cbf6367ef756c355f0e3dcec783e11a99660cc8b`
ran once. It failed closed with measurement code `THRESHOLD` and preserved the
exact immutable root
`.9f22e22e41972db56fbca6d8cbf6367ef756c355f0e3dcec783e11a99660cc8b.qualification-failed-2232586-0`.
Its empty report is correctly retained as zero bytes. The resource receipt
records 17,009,988,205 elapsed ns and 20,766,720 maximum RSS bytes, but these
operational totals do not identify which accepted threshold failed. Because
the receipt's message discarded the class and observed value, no performance
result or remediation can honestly be inferred from this run. It must not be
rerun.

The bounded replacement changes observability only. The exact same evaluator
predicates and limits remain in the same deterministic order: logical sequence,
logical extras, latency, allocations, heap, storage, then pages. A structured
rejection carries the first failed class and a stable bounded ASCII message
with all observed/limit values for that compound class. Valid identities and
normal page vectors are literal. Malformed identity text is redacted to byte
length plus SHA-256, and only a page vector too large for the accepted 256-byte
message uses length plus the SHA-256 of its u64-big-endian contents. Thus one
future reviewed candidate can fully diagnose its first actionable blocker
without a diagnostic rerun. Measurement failures still retain an empty report,
and neither the failure schema nor qualification policy changes.

The first implementation incorrectly used only a debug assertion for the
256-byte invariant; maximal `u64` values plus an invalid identity could exceed
it in release builds. The corrected logical grammar is compact:

- `logical-sequence b=<observed>/<limit> s=<observed>/<limit> c=<observed>/<limit>`;
- `logical-extras n=<observed>/<limit> s=<observed>/<limit>`.

The common constructor now enforces the bound in every build: an unexpectedly
non-ASCII or overlong diagnostic becomes an exact
`<class> diagnostic_len=<bytes> diagnostic_sha256=sha256:<digest>` content
identity rather than ambiguous truncation. Maximal numeric values, a large
invalid identity, a 4,096-value page vector, and the actual failure sealer prove
that every returned diagnostic remains ASCII, bounded, attributable to its
class, and publishable with an empty report.

Focused construction-bound remediation gates passed: all-feature Clippy;
18 index tests; 39 build library/reference/resource tests; 18 feature-gated
benchmark tests; 20 executable reference specs; and `git diff --check`. No
qualification, candidate, or production build path ran, and neither retained
evidence root was modified.

## Observable latency failure and bounded reader remediation

The independently approved observable replacement contract
`09ea27dde9169eb214b6d3a7abc1de7b139c1883fb0112573959a8a56be88887`
ran once and preserved the immutable root
`.09ea27dde9169eb214b6d3a7abc1de7b139c1883fb0112573959a8a56be88887.qualification-failed-2390227-0`.
Its closed failure receipt identifies latency as the first failed class: p50
was `7485 ns` against `5586 ns`, and p95 was `11984 ns` against `6100 ns`.
The report remains empty as required. Since the evaluator stops at latency,
allocations, open heap, storage/Zstandard, and pages were not accepted or
rejected and no result for those later classes may be inferred. The v1 format,
production member, provider API, and thresholds are unchanged.

The threshold values are arithmetically 25% above Ticket 010's retained
`4469/4880 ns` headline (`5586/6100 ns`, with the p50 fractional result reduced
to the recorded integer limit). That arithmetic is an inference about how the
limits were chosen, not evidence that the two measurements were run under
equivalent conditions. Ticket 010 retained CPU 0, AMD Ryzen 7 5825U,
`powersave`, Linux `6.17.0-35-generic`, and Rust 1.93.1. The failed v2 evidence
does not durably establish affinity, CPU, governor, or kernel. A future
coordinator-authorized candidate must pin CPU 0 and retain those facts before
its latency is compared with Ticket 010.

The timed method is close but not identical. Both paths run five rounds, 20
warmups per round, and 10,000 cyclic M01–M14 copies; compute nearest-rank p50
and p95 per round; and retain the median round headline. V2 verifies all 14
contexts before each round, while Ticket 010 performed exactness preflight
outside its retained round loop. V2's additional verification warms the query
data, so this difference does not explain a slower result as cold-page work.
No timing improvement is claimed from the remediation below.

Source inspection identified a concrete production regression. Ticket 010's
selected `acgt2-rle-v1` decoder copied an unaligned prefix, decoded every
aligned packed byte into four bases with one table lookup and slice copy, then
copied the tail. The production decoder instead repeated checked addition,
division, bounds lookup, and table indexing for each base in every approximately
10.1-kilobase compatibility context. The qualification feature also updated a
thread-local logical-read counter on every timed copy although the harness used
that counter only to prove that construction did not inspect dense bytes.

The production decoder now uses the selected aligned four-base algorithm with
upfront arithmetic and packed-length validation, preserving the caller's
destination on malformed input. The query-path counter has been removed.
Qualification instead asks the constructor for an explicit dense-read count
calculated from the constructor's bounded mmap ranges, so timed copies execute
the ordinary production provider path.

Full-data-free controls compare the optimized decoder directly with the old
scalar reference over every start in a 1,024-base bounded packed vector and
every length from 1 through 33 that fits. Separate controls cover all four
start alignments; lengths 1, 2, 3, and 4 plus aligned groups and tails; an
actual packed read crossing a 4,096-byte mmap page; ambiguity runs before,
inside, after, and enclosing windows; corrupt and out-of-bounds unchanged
destinations; concurrent copies; constructor-only dense-read auditing; and a
dedicated warmed zero-allocation copy test. These establish compatibility and
remove known artificial timed work. A later retained qualification remains the
only performance acceptance evidence.

Focused remediation gates passed: all-feature Clippy; 22 index library tests;
40 build library/reference/resource tests (24/15/1); 18 feature-gated
qualification-binary tests; 20 executable reference specs; formatting; and
`git diff --check`. No qualification, candidate, production build, or timing
workload ran during remediation, and retained production evidence was not
modified.

## Retained optimized qualification success

After independent qualification-readiness approval, the single authorized
optimized reuse contract
`edaee0374147b70a5259a4a2f0f4120914232f018531391e9625438fd2db35af`
ran once on the preserved build and passed every unchanged threshold. The exact
retained success root is
`/home/ian/workspace/data/pangopup-reference-production-011/edaee0374147b70a5259a4a2f0f4120914232f018531391e9625438fd2db35af/`.
Its launcher stdout and stderr are both empty, and the canonical report has
`passed=true`.

The retained result is:

- headline p50 `1864 ns` and p95 `2084 ns`;
- allocation calls/bytes `0/0`;
- open peak Rust heap `16060` bytes and dense bytes read during open `0`;
- benchmark maximum RSS `20938752` bytes and benchmark minor/major faults
  `4738/0`;
- installed bundle `772096272` bytes;
- unchanged reference member `772091760` bytes;
- pinned `zstd-0.13.3/libzstd-1.5.7`, level 9, single-threaded measurement size
  `656781805` bytes;
- five rounds, 20 warmups per round, 10,000 cyclic operations, nearest-rank
  quantiles, with round p50 values `1863,1874,1854,1864,1884 ns` and round p95
  values `2105,2084,2004,2084,2114 ns`;
- per-case page-count sum 20 and exact unique dense pages `31748,109204,119053,
  119054,133714,133715,152494,152495`.

All logical evidence is unchanged: 25 contigs, `3088286401` bases, 14 verified
contexts, 680 ignored records, sequence-set SHA-256
`2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4`,
and extra-accession SHA-256
`0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb`.
The report retains AMD Ryzen 7 5825U, x86_64 Linux
`6.17.0-35-generic`; the launch checkpoint records the reviewed CPU0-pinned
`powersave` execution.

The prior successful full build and both immutable qualification failure roots
remain preserved. The optimized run reused the existing production member; no
second full reference build occurred. This success establishes the v1
reference provider and bundle against its existing thresholds. It does not
publish or transport the reference asset and does not add model or mask data.
