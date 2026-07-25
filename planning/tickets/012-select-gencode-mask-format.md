# 012 — Select exact GENCODE mask semantics and a compact mmap format

Status: complete
Base revision: `b2a8d21490196930babd4e05055336fb91101708`

## Why

Pangopup now has a qualified production GRCh38 sequence provider, but model
fallback also needs the gene strand, containing-gene order, and exon boundaries
used by Pangolin masking. The existing frontier incorrectly combined that work
with checkpoint packaging.

The mask is not an ordinary coordinate set. In Pangolin 1.0.2,
`get_genes()` iterates the result of a gffutils point query into insertion-
ordered Python dictionaries. `process_variant()` then mutates one shared
gain/loss array while visiting each same-strand gene. A later overlapping gene
therefore observes mutations made for an earlier gene. Sorting a compact mask
by coordinate or stable Ensembl identifier can silently change scores.

Ticket 009 proves this behavior with controlled cases, but its retained
artifact explicitly does not represent complete all-gene order. It also exposes
a type boundary that must close before a format is frozen: GENCODE uses
versioned identifiers such as `ENSG00000141510.18` and pseudoautosomal copies
such as `ENSG00000228572.7_PAR_Y`, while the shipped Zenodo lookup intentionally
stores an unversioned 15-character `EnsemblGeneId`.

The observable outcome of Ticket 012 is a checked miniature source/oracle,
complete production-data semantics inventory, fair measured comparison of
plausible mmap candidates, and an accepted format/identity ADR. It is the mask
equivalent of Ticket 010. It does not create the production bundle or model.

## Pinned facts

- Compatibility profile: `pangolin-1.0.2-5cf94b8-grch38-v1`.
- Pangolin source commit:
  `5cf94b8db938c658391b4305cd7ce33297d44ff7`.
- Upstream annotation database:
  `gencode.v38.annotation.db`, 380,366,848 bytes, SHA-256
  `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6`.
- GENCODE source:
  `gencode.v38.annotation.gtf.gz`, 46,556,621 bytes, SHA-256
  `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050`,
  official MD5 `16fcae8ca8e488cd8056cf317d963407`.
- Upstream database construction filters transcript/exon features to tag
  `Ensembl_canonical`, retains gene features, and uses gffutils 0.14 with gene
  and transcript inference disabled.
- Upstream point semantics are `gtf.region((contig, pos - 1, pos - 1),
  featuretype="gene")`, followed by a conventional inclusive GTF-span check
  against `pos` and `gtf.children(gene, featuretype="exon")`. Together these
  make Pangolin's effective containing domain `(gene_start, gene_end]`: the
  exact gene start is excluded and the exact gene end is included.
- gffutils 0.14's region query has no SQL `ORDER BY`, and gffutils documents its
  default order as unspecified. Database bytes and package version alone do not
  pin the order. Ticket 012 must freeze an authenticated upstream observation,
  including SQLite runtime/query-plan identity, as a canonical ordered export.
- A read-only pre-ticket database inventory observed 60,649 genes and 44 exact
  `_PAR_Y` identifiers, producing 44 stable-ID collisions with chrX records.
  Ticket 012 must reproduce these counts from its authenticated capture rather
  than treating the exploratory values as authority.
- That review probe used the exact local database/GTF identities above and
  observed Python 3.13.5, gffutils 0.14, sqlite3 module 2.6.0, SQLite runtime
  3.49.1, database last-written SQLite 3.35.4, no sidecars, sorted
  `sqlite_master` schema SHA-256
  `99a2bb9a60b4f425dcbf0a497355ea9a204a6d38b9abf69e714db3ef252f7a49`,
  and plan `SEARCH features USING INDEX seqidstartend (seqid=?)`. These are
  investigation facts, not the canonical export or a substitute for Ticket
  012's authenticated observation.
- ADR 0008 requires exact observed same-strand order for the strict profile. A
  corrected independent-per-gene policy is a different future profile.
- The source data are build inputs only. The selected runtime representation
  must not require Python, GTF parsing, SQLite, or gffutils.
- Query speed is the first optimization priority after exactness; memory/pages
  are second and compressed download size third.

## Scope

### Included

- A synthetic, checked-in annotation fixture and independent expected oracle
  that cover versioned identifiers, an exact chrX/chrY `_PAR_Y` pair with one
  stable-ID collision, both strands, overlapping same-strand and opposite-
  strand genes, shared and distinct exon boundaries, `start`, `start+1`, `end`,
  and `end+1`, coordinate 1 without underflow, no containing gene, empty
  canonical exon sets, duplicate/reordered source facts where relevant, and all
  supported primary contig aliases.
- A documented logical mask contract containing the minimum facts needed by
  Pangolin: exact versioned/PAR gene identity, non-unique stable unversioned
  component, contig, strand, stored inclusive GTF span, effective `(start,end]`
  point domain, ordered point-query rank, and the semantically required exon-
  boundary set.
- A closed decision for how versioned GENCODE identity coexists with the current
  unversioned Zenodo lookup identity and `--gene` filter. Model-facing output
  must retain the exact version and optional `_PAR_Y` suffix. Stable filtering
  matches every exact identity with that stable component on the request's
  already selected contig; it never merges chrX and chrY records or treats the
  stable component as globally unique.
- One private feature-gated candidate binary, not a new public
  `pangopup-build` command family. Its complete `--help` and closed internal
  command grammar cover authenticated capture, candidate preparation,
  inspection, point query, and benchmark. Focused executable specs may invoke
  this binary, but it is excluded from ordinary installation and does not
  repeat or worsen the public maintainer-help debt.
- A Rust-orchestrated, bounded upstream observation that authenticates the
  pinned database and GTF through held no-follow inputs, privately snapshots the
  exact bytes that will be queried, rejects `-wal`, `-shm`, and journal sidecar
  state, and rejects change before or during capture. The base interpreter and
  generic venv launcher are distinct authenticated inputs: the held exact base
  descriptor executes with a held-prefix launcher, while the launcher symlink
  relationship and derived `pyvenv.cfg` are rebound before and after each
  child. `-S` and an explicit held-prefix site-packages path bypass `.pth`
  startup code; bytecode lookup is redirected away from existing caches. The
  helper records Python executable/version/prefix/base facts, exact helper and
  every loaded file-backed or interpreter-owned module identity, gffutils
  version, built-in `_sqlite3` identity, SQLite library version and compile
  options, exact region query shape, schema/index identity, and relevant
  `EXPLAIN QUERY PLAN` output. File module evidence includes content plus exact
  device, inode, hardlink count, size, mtime, and ctime, so uv hardlinks are
  accepted without making mutation invisible.
- One canonical ordered logical export produced by executing the exact upstream
  point and child queries in that recorded environment over every constant-
  membership domain. Candidates consume that export; neither a new Rust SQL
  query nor a digest derived from candidate output is allowed to invent the
  expected order.
- A complete production inventory that enumerates genes, strands, canonical
  exon boundaries, versioned/PAR/stable identifier relationships and
  collisions, duplicate-ID behavior, distinct effective overlap domains, and
  every same-strand order-sensitive set. It records a canonical logical-stream
  digest and explicit counts sufficient to detect later sorting, filtering,
  containment, or PAR drift.
- An independent full-domain comparator that queries every candidate at one
  witness in every constant-membership domain plus `start`, `start+1`, `end`,
  and `end+1` edges and compares ordered identities/boundaries directly to the
  canonical upstream export. This exhaustive correctness set is distinct from
  the weighted performance manifest.
- At least three exact candidate layouts behind a benchmark-only format family:
  one simple direct interval baseline, one compact indexed candidate, and one
  materially different candidate justified by observed distribution. Every
  candidate uses the same query API, fixture, full logical stream, query
  manifest, output ownership, and measurement harness.
- A read-only mmap candidate reader that returns all containing genes in exact
  upstream order, separated by strand, with exact boundaries and no payload-
  wide query scan. Candidate readers use checked decoding and own the mmap;
  consumers never cast mapped bytes or know offsets.
- One retained full-source candidate build/inventory and one retained optimized
  comparison under the closed lifecycle below. It is not a normal gate and is
  not repeated to verify a hash.
- A selected-format ADR and retained
  `planning/artifacts/012-gencode-mask-format-selection.md` with exact inputs,
  commands, host/toolchain, candidate identities/sizes, semantic counts/digest,
  query manifest, round results, allocations, RSS/pages, compressed sizes, and
  the mechanical selection result.
- Artifact-specific builder-source identity for the new mask candidates. Do not
  bind their identity to unrelated release, sync, compatibility, or SNV code.
- Update `README.md`, `architecture/README.md`, `architecture/design.md`,
  `architecture/runtime-data.md`, `architecture/index.md`,
  `architecture/delivery.md`, `planning/faq.md`, and `planning/frontier.md` so
  current versus future behavior and the selected representation are accurate.
  Reconcile the stale claims listed in
  `planning/issues/2026-07-24-maintainer-interface-and-documentation-drift.md`
  where this ticket touches them; unrelated maintainer-help work remains in the
  issue.

### Excluded

- A production mask bundle/provider, production format magic, XDG installation,
  transport, remote sync, GitHub release, or publication.
- Packaging, converting, loading, or running Pangolin checkpoints; choosing a
  tensor runtime; numeric tolerance; CPU/MPS/CUDA/ONNX inference.
- Changing the shipped SNV score bundle, its unversioned source-gene semantics,
  the production reference format/provider, or any retained large artifact.
- A public general gene-annotation API, gene aliases/descriptions, HGVS,
  transcript/protein projection, GRCh37, or liftover.
- Lookup-first routing, non-SNV public input, caching, HTTP, Docker, systemd,
  executable publication, or repository-setting changes.
- Repairing release-upload subprocess lifecycle. That is a separately recorded
  blocker before the next public asset upload.
- A recurring complete-source verifier. Normal tests use only the miniature and
  retained small summaries.
- Repairing the public `pangopup-build` root/nested help and version behavior.
  Ticket 012 adds no command to that public binary; the separate issue remains
  the owner.
- Drafting Ticket 013.

## Success Checklist

- The fixture proves exact `(start,end]` point membership at `start`, `start+1`,
  `end`, and `end+1`, safe coordinate-1 behavior, and deterministic plus/minus
  ordered results for overlapping genes, including a case where changing same-
  strand order changes masked output under ADR 0008 replay.
- The logical contract retains the full versioned/PAR identifier, models the
  44 observed stable collisions without assuming global stable-ID uniqueness,
  and defines contig-disambiguated stable filter behavior without changing
  existing lookup results.
- Exact source authentication happens before production scanning. Malformed,
  changed, wrong-schema, wrong-filter, unsupported-contig, duplicate, overflow,
  noncanonical, sidecar-active, and observation-environment/query-plan inputs
  fail closed with bounded error output.
- The complete production inventory is streamed with bounded heap and proves
  the pinned upstream ordered output over every distinct effective overlap
  domain. It does not claim that the database hash, a Rust SQL query, or selected
  compatibility cases alone establishes order.
- Every candidate round-trips the independent miniature and matches the full
  canonical logical digest/counts. Corrupt headers, directories, intervals,
  ranks, IDs, boundaries, offsets, truncation, and trailing bytes fail closed.
- Candidate open performs bounded metadata work rather than a full payload scan;
  a warmed point query has no unexpected heap allocation and touches only the
  indexed regions required by that query.
- The retained benchmark uses the separate predeclared 1,000-query performance
  manifest defined below. It records warm p50/p95, allocations, open time/heap,
  deterministic page behavior, RSS/faults, member size, and pinned single-
  threaded Zstandard size for every exact candidate.
- The closed selection procedure below yields exactly one winner without a
  pairwise or non-transitive tie interpretation.
- Production data, candidate outputs, SQLite/GTF inputs, and benchmark binaries
  remain outside Git. Only the miniature, code, specs, ADR, and bounded retained
  report/query manifest are committed.
- `make lint`, `make test`, and `make spec` pass without Python, gffutils,
  production annotations, checkpoints, a network request, or an exhaustive
  production scan.

## Decisions

### Behavioral authority and reproducibility

For strict compatibility, the authority is the canonical ordered export from
one authenticated execution over the exact pinned upstream SQLite bytes in the
fully recorded Python/gffutils/SQLite environment. The database hash by itself
does not pin unordered SQL results. Candidate expected values come from this
export and are compared directly over every effective constant-membership
domain.

The exact GTF, upstream filter/create script, and gffutils version remain
provenance and reproduction inputs. A database rebuilt from the GTF must match
the complete gene/span/strand/exon fact multiset; a fact mismatch fails Ticket
012. Query-order differences are recorded but do not replace the canonical
upstream export, because unordered SQLite plans may legitimately differ. If the
authenticated upstream export itself cannot be reproduced consistently within
the recorded observation environment, no format is selected.

This is deliberately narrower than making SQLite a runtime dependency. The
production inspector/exporter may read authenticated SQLite in the build crate;
every candidate and future runtime member is closed Rust-owned data.

### Versioned gene identity

The mask's exact identity grammar is `ENSG` plus 11 decimal digits, a dot, a
nonzero decimal version without a leading zero, and optional literal suffix
`_PAR_Y`. Its stable component is the 15-character `ENSG` identifier. Exact and
stable identities are distinct types; the stable component is not globally
unique. Full exact identities, including version and PAR suffix, are never
collapsed inside the mask logical stream.

A current unversioned `--gene` filter remains a stable-ID filter. On a concrete
request contig it matches every containing mask record with that stable
component; result identity/provenance retains the exact full identifier. It
does not join or merge chrX and chrY PAR copies. A future exact-version filter,
if useful, is a separately named input rather than an ambiguous extension of
the current parser.

The existing `EnsemblGeneId` and fixed-v1 SNV bytes remain unchanged. Ticket
012 records the future public type/API consequence in its ADR; it need not add
model result types before a model provider exists.

### Ordered semantic stream

The canonical logical stream is independent of candidate byte layout. It is
framed by primary contig order and every exact versioned/PAR gene record needed
to reproduce upstream point results: observed query order, exact/stable
identity, strand, stored inclusive GTF span, effective `(start,end]` domain,
and semantically normalized exon boundaries. Before
choosing normalization such as sorting/deduplicating boundaries, the
implementation must prove from upstream masking math and the complete source
that it preserves outputs. Gene ordering is never normalized away.

The full proof partitions each contig by effective activation at `start+1` and
deactivation at `end+1`, where the containing ordered set is constant, rather
than testing every one of three billion positions. It checks one interior
witness per nonempty domain plus every distinct `start`, `start+1`, `end`, and
`end+1` that is in coordinate range. Position 1 is handled without unsigned
underflow. Candidate membership must match this upstream behavior rather than
conventional inclusive interval lookup.

### Candidate-only now, production hardening next

Ticket 012 uses a distinct benchmark-only container and selects a payload. The
next coordinator-authored ticket may harden only the winner with production
magic, manifest, NOTICE, typed provider, full builder, and qualification.
Keeping selection and hardening separate prevents another broad one-shot asset
ticket and keeps model representation independent.

### Measurement and retained evidence

The full annotation scan and optimized benchmark are one-time acceptance
evidence, not routine verification. Normal gates replay a small independent
fixture and retained identities. The coordinator must preserve successful
outputs and must not rerun the full source merely to compute or recheck hashes.

No candidate may win by returning borrowed data while another returns owned
data, using a different query set, preloading the whole member, omitting exact
version/order facts, or measuring serialization only for one candidate.

The exhaustive effective-domain manifest is correctness evidence only. It does
not weight the performance test toward rare overlaps. A separate deterministic
1,000-query performance manifest has these exact strata, in this order:

```text
486  single-gene interior points, proportionally apportioned by contig
100  no-gene points between observed domains
100  same-strand multi-gene domain witnesses
100  opposite-strand multi-gene domain witnesses
100  boundary probes: 25 each of start, start+1, end, end+1
 88  one interior chrX/chrY witness for each of the 44 PAR stable-ID pairs
 26  fixed compatibility/extreme-cardinality witnesses
```

For the 486 single-gene entries, let `n_i` be the count of distinct eligible
single-gene effective domains on contig `i`, and `N=sum(n_i)`. Give each contig
`floor(486*n_i/N)` slots, then assign remaining slots by descending remainder
`(486*n_i) mod N`, breaking ties by contig code. Within a contig with `q` slots,
sort domains by coordinate and choose zero-based index
`floor((2*j+1)*n_i/(2*q))` for slot `j=0..q`; checked arithmetic is mandatory.

For the 26 final entries, use M01–M14's genomic points in case order, followed
by witnesses from the 12 effective domains with the highest ordered containing-
gene count, breaking ties by contig, coordinate, then exact ordered gene IDs.
Duplicates with another stratum remain intentional workload entries. For every
other stratum, sort eligible observations by contig code, coordinate, and exact
ordered gene IDs and take the first required count. If such a stratum has too
few distinct points, cycle that sorted list and record distinct/repeated counts;
zero eligible observations invalidates the run. The manifest records every
request and expected ordered-result digest and is finalized before timing.

The optimized harness is one feature-gated release binary and uses:

- one verified logical CPU selected from the inherited allowed affinity mask;
  it sets and rechecks single-CPU affinity and records CPU model, kernel,
  governor/power state, rustc target/version, and logical page size;
- six measured rounds, each with one long-lived mmap reader per candidate;
- 10,000 untimed warmup queries then exactly 100,000 timed point queries per
  candidate/round, cycling the 1,000-query manifest in file order;
- all six candidate block permutations in this fixed order: `A/B/C`, `A/C/B`,
  `B/A/C`, `B/C/A`, `C/A/B`, `C/B/A`, where A/B/C are the fixed candidate
  identifiers declared before the run. Each candidate appears exactly twice in
  every schedule position;
- buffers and timing storage allocated/touched before counters reset, identical
  caller-owned scratch/output capacity for all readers, and black-boxed result
  identity/count outside the timed interval;
- nearest-rank per-round p50/p95 (`ceil(p*n)-1`) and the nearest-rank p50 of the
  six round quantiles—zero-based sorted index 2—as each headline;
- open nanoseconds and peak outstanding Rust heap per round; warmed allocation
  calls/bytes per query family; deterministic metadata/payload 4,096-byte page
  traces per query; process maximum RSS and minor/major faults as retained
  observations rather than pass thresholds; and
- complete member bytes plus deterministic Zstandard bytes using the repository
  pinned `zstd 0.13.3` / `zstd-safe 7.2.4` / bundled libzstd 1.5.7, level 9,
  checksum/content-size enabled, dictionary ID and long-distance matching
  disabled, zero workers, and exact pledged input size.

Selection is a closed filtering procedure over candidates that already passed
every semantic and corruption control:

1. Find the minimum headline p95. Retain candidates satisfying
   `candidate_p95 * 100 <= minimum_p95 * 105` with checked `u128` arithmetic.
2. Within those survivors find the minimum headline p50 and retain candidates
   satisfying the equivalent five-percent p50 inequality.
3. Retain the smallest median logical payload-page count per performance query,
   then the smallest p95 logical page count, then the smallest maximum open
   peak heap, then the smallest complete member bytes, then the smallest pinned
   Zstandard bytes.
4. If an exact tie remains, select the first candidate in the fixed simplicity
   order recorded and justified before timing. That order may rank only format
   complexity/checking burden; it may not use a measured result.

Every filtering step and survivor set enters the canonical report. Invalid
arithmetic, environment drift, oracle mismatch, allocation-contract mismatch,
or missing evidence produces no selection.

### Source and artifact limits

Production accepts only the exact input sizes/hashes above. General parsing and
fixture failure paths additionally enforce: at most 25 primary contigs, 100,000
genes, 10,000,000 exon-boundary values, 100,000 boundaries for one gene, 64
bytes per exact gene ID, 1 GiB canonical export, 512 MiB per candidate member,
64 KiB canonical manifest/report metadata object, and 4 KiB sanitized error
text. All offset/count products use checked arithmetic and allocation occurs
only after aggregate limits pass.

Input paths are absolute explicit maintainer inputs. Open each input once with
no-follow semantics, require a regular file, and snapshot it into a private
mode-0700 staging directory while hashing from the held descriptor. Query only
the immutable private snapshot; never reopen the caller's name. Reject source
directories containing database `-wal`, `-shm`, or journal state and reject any
snapshot identity change. Candidate directories are private, bounded, closed,
and published by same-parent no-replace rename only after inspection. No input,
output, temp, database, query, or OS error path is emitted in public JSON.

### One-time production lifecycle

1. The developer implements capture, miniature oracle, candidates, readers,
   comparator, benchmark, failure injection, internal executable specs, and
   documentation; all focused miniature tests and normal gates pass without
   production inputs.
2. The independent code reviewer reviews the causal code, exact source/helper
   inventory, limits, fixture independence, candidate fairness, report schema,
   run command, cancellation/failure policy, and selection evaluator. It records
   `RUN-READY` before any production capture/build/benchmark is authorized.
3. The coordinator records the exact reviewed diff/contract, optimized binary,
   Python/gffutils/SQLite/GTF/database inputs, host/affinity, free space,
   command, absent output root, staging root, process/session, progress files,
   and cancellation procedure. It then performs the single authenticated local
   production capture/build/benchmark. No network or external publication is
   permitted.
4. The developer records only the bounded generated report/query manifest,
   selected ADR, artifact evidence, and current/future documentation. A policy,
   codec, oracle, evaluator, or causal-code change invalidates `RUN-READY` and
   returns to step 1; evidence transcription alone does not.
5. The same code reviewer reviews the preserved production output identities,
   selection, bounded checked evidence, documentation, and final diff and must
   record final acceptance.
6. The coordinator performs the final stale-claim scan and `make lint`,
   `make test`, and `make spec`, then commits/pushes only after acceptance.

The output root had to be absent before the first authorized launch. Its
private directory and immutable failure-only children are not deleted after a
recorded preflight failure. A newly reviewed candidate may reuse the parent
only while that candidate's deterministic preflight-failure child and final
contract child are both absent; earlier failure-only siblings may coexist.
Work occurs in a uniquely
named private, contract-addressed sibling stage and publishes no-replace only
after successful capture, semantic comparison, measurement, and report sync. Capture/export,
candidate build plus exhaustive certification, and benchmark/report are three
explicit phases. Each completed phase atomically writes and syncs a canonical
receipt binding its inputs, causal code, outputs, member hashes, counts, and
next allowed phase.

Any handled failure preserves the complete mode-0700 stage plus a sanitized,
synced failure receipt naming the phase; it does not delete a completed export,
candidate set, or other expensive authenticated evidence. The failed stage is
never treated as success or selected by runtime code. After causal diagnosis,
the coordinator and same code reviewer may authorize a new absent stage to
reuse only a phase whose receipt and every held member reauthenticate exactly;
the new run records the prior receipt identity. Partial/unsealed phase output
is evidence only and is never resumed. Failure is never automatically retried.
An unchanged successful output is never rerun. A causal policy/code/input change
requires a new contract and the full review lifecycle; a transient late failure
may reuse only already sealed unchanged phases under the authorization above.

The pre-final-contract boundary is explicit. After CLI syntax, declared
identity, and output-parent validation, any handled capture-preflight failure
creates one failure-only mode-0700 sibling named
`.pangopup-mask-preflight-failure-$PREFLIGHT_ID`. The ID is the SHA-256 of a
domain-separated canonical contract containing the declared database, GTF,
base-Python, launcher-link, `pyvenv.cfg`, helper, policy, and causal-code
identities but no local paths. Its sole mode-0400 `failure.json` nests that
contract and bounded sanitized error. It contains no annotation snapshot,
observation, or phase receipt, cannot be mistaken for success, and an identical
automatic retry is refused without overwriting it. CLI syntax/invalid-pin
errors and an inaccessible output parent precede the first trustworthy durable
destination and therefore cannot create such evidence.

## Dependencies

- Ticket 009: strict upstream compatibility corpus and ADR 0008 — complete.
- Ticket 011: production RefSeq GRCh38.p14 reference provider — complete.
- `planning/issues/2026-07-21-overlapping-gene-mask-order.md` — remaining work
  promoted into this ticket.

## Notes

- The production inputs are not currently checked into the repository. Their
  exact URLs, sizes, and digests are in
  `tests/fixtures/pangolin-compat-v1/manifest.json`; accept explicit local paths
  and never download implicitly from a builder or test.
- The authenticated observation helper necessarily executes the exact pinned
  Python/gffutils/SQLite behavior whose unordered output is being frozen. Rust
  owns authentication, bounded orchestration, canonical export inspection,
  candidate building/readers, comparison, and benchmark. Python/gffutils never
  enters normal gates or runtime.
- Do not reuse the over-broad existing `PANGOPUP_BUILDER_SOURCE_SHA256` for the
  new candidate identity. Define a documented, minimal source inventory for
  mask semantics/codec code and its manifests.
- Reuse existing checked mmap and bounded parsing primitives where their trust
  contracts fit. Do not copy local-install, remote-sync, or release-publication
  machinery into this ticket.
- The complete all-gene proof must say exactly what was compared. A database
  file hash plus a handful of spot checks is not semantic certification.
- No production capture, build, or benchmark begins before the same future code
  reviewer records `RUN-READY`. The coordinator alone authorizes the retained
  run, and the reviewer must subsequently accept its evidence and final diff.

## Coordinator Authorship

Coordinator: `/root`

Drafted from the shipped Ticket 011 outcome, the 2026-07-24 adversarial project
review, ADR 0008, and the rolling frontier. The coordinator has not implemented
the ticket or run production GENCODE work.

## Independent Ticket Review

Reviewer: `/root/planning_release_review`

First review verdict: `REJECT`. The reviewer found five Major and two Minor
contract defects: unordered SQLite output was not environmentally/canonically
pinned; `_PAR_Y` IDs and 44 stable collisions were omitted; conventional
inclusive containment contradicted Pangolin's effective `(start,end]` domain;
benchmark ranking and retained-run lifecycle were open; and the ticket added
public commands despite known help drift. It also required separate correctness
and performance manifests plus explicit held-source limits.

The coordinator accepted every finding. This revision adds the authenticated
ordered export and GTF-rebuild policy, full PAR identity/filter rules, exact
effective-domain proof, closed workload/measurement/selection/lifecycle,
private feature-gated tooling, separate manifests, and numeric/no-follow source
limits.

Second review verdict: `REJECT`. The seven original findings were closed, but
the five-round schedule was not position-balanced, two performance strata were
not generated by a complete deterministic rule, and ordinary failure cleanup
could discard completed expensive phases. The coordinator accepted those two
Major findings.

Final review verdict: `ACCEPT`. The reviewer re-read the final substantive
revision in the shared worktree. All six candidate permutations are balanced;
headline aggregation, largest-remainder/within-contig sampling, and the final
14+12 witnesses are closed. Failed contract-addressed stages and sealed phase
receipts are preserved; reuse is explicit, reauthenticated, coordinator-
authorized, and same-reviewer-authorized, while partial phases and automatic
retry are forbidden. The reviewer found no remaining Major or Minor defect,
made no edits, and is ineligible to implement Ticket 012. The coordinator then
changed only this review record and status from `proposed` to `ready`.

Planning-ready gate: `make lint`, `make test`, and `make spec` passed on the
planning diff on 2026-07-24; 143 executable specs passed. No production input,
capture, candidate build, or benchmark ran.

## Implementation Evidence

Developer: `/root/ticket012_developer2`

Pre-production implementation was completed against reviewed ticket SHA-256
`997abe87caa29fe8f2780d8fd85e317d489d950170eaebe1b189774ba2a1d2c9`
at accepted planning commit `ff5b5e23b9ea6146045c4275de053db605a04727`.
`/root/ticket012_developer` authored only a superseded partial exact-identity
draft. The current developer independently reviewed and corrected that draft
before taking ownership of the complete implementation diff.

The implementation adds the distinct checked `GencodeGeneId`; the independent
miniature; three `PGMBEN01` writers/readers; exhaustive logical and point-query
certification; exact stable/PAR filter semantics; bounded authenticated
Python/gffutils/SQLite capture; independent GTF fact comparison; deterministic
correctness/performance manifests; the six-round selector; allocation/page/RSS/
fault/member/Zstandard evidence; cancellation; sealed phase/failure/reuse
receipts; held no-follow source and candidate authentication; and held
same-parent no-replace publication. The feature-gated binary is private and is
not installed as part of the supported product CLI.

Focused pre-review evidence on 2026-07-24:

- all nine `pangopup-build::mask` lifecycle/selector tests passed;
- all eight `pangopup-index` mask-candidate format/corruption tests passed; and
- strict Clippy for the feature-gated executable passed with warnings denied.

The full pre-review gate then passed: `make lint`, `make test`, and `make spec`
were green, with 146 executable specs. The first `make test` correctly exposed
that the legacy repository-wide SNV builder fingerprint had changed. The
checked source-derived generator refreshed only the miniature bundle identity
and the repeated provenance `bundle_id`; source rows, requests, and scores were
unchanged. The byte-for-byte regeneration test passed on the next full run.

No production GENCODE annotation scan, candidate build, compression, or
benchmark has run. Two authorized capture preflights failed as recorded below:
first the bare-interpreter import, then the unnormalized `sqlite3.Row`. The
row-normalizing replacement is under re-review. The corrected launch retains a
separately pinned, generic Python environment alongside the unchanged
database/GTF/base-interpreter inputs:

```text
database=/home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db
database_bytes=380366848
database_sha256=221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6
gtf=/home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.gtf.gz
gtf_bytes=46556621
gtf_sha256=22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050
python=/home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13
python_bytes=34679464
python_sha256=c243a3ad6dc86fcde244245aca621adee9766759c7524ca89f1b3a44ff4fdc24
python_launcher=/home/ian/.local/share/uv/tools/pangolin/bin/python
python_launcher_kind=symlink
python_launcher_target=/home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13
python_launcher_link_bytes=79
python_launcher_link_sha256=b407404b75f49e4f39686a8a060bdcbadae49e5c5f1ecd5d6e593f9468bc8ffe
python_prefix=/home/ian/.local/share/uv/tools/pangolin
python_base_prefix=/home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu
pyvenv_config=/home/ian/.local/share/uv/tools/pangolin/pyvenv.cfg
pyvenv_config_bytes=171
pyvenv_config_sha256=b39b62bae0935628201c24541bebd3011ff5527b55070543a4c565542d8b2ba9
output_parent=/home/ian/workspace/data/pangopup-mask-qualification-012
output_parent_state=exists_with_preserved_failure_only_stage
```

After an independent reviewer records a new `RUN-READY`, the coordinator may
reuse the preserved private output parent and run exactly:

```bash
set -euo pipefail
cd /home/ian/workspace/repos/pangopup
install -d -m 700 /home/ian/workspace/data/pangopup-mask-qualification-012
cargo build --locked --release --package pangopup-build \
  --features mask-qualification --bin pangopup-mask-candidates
BIN=/home/ian/workspace/repos/pangopup/target/release/pangopup-mask-candidates
OUT=/home/ian/workspace/data/pangopup-mask-qualification-012
CAPTURE_JSON=''
if CAPTURE_JSON="$("$BIN" capture \
  --database /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db \
  --gtf /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.gtf.gz \
  --python /home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13 \
  --python-bytes 34679464 \
  --python-sha256 c243a3ad6dc86fcde244245aca621adee9766759c7524ca89f1b3a44ff4fdc24 \
  --python-launcher /home/ian/.local/share/uv/tools/pangolin/bin/python \
  --python-launcher-link-bytes 79 \
  --python-launcher-link-sha256 b407404b75f49e4f39686a8a060bdcbadae49e5c5f1ecd5d6e593f9468bc8ffe \
  --pyvenv-config-bytes 171 \
  --pyvenv-config-sha256 b39b62bae0935628201c24541bebd3011ff5527b55070543a4c565542d8b2ba9 \
  --output-parent "$OUT")"; then
  printf '%s\n' "$CAPTURE_JSON"
else
  status=$?
  printf '%s\n' "$CAPTURE_JSON"
  exit "$status"
fi
CONTRACT_ID="$(printf '%s\n' "$CAPTURE_JSON" | jq -er '.contract_id')"
STAGE="$OUT/.pangopup-mask-stage-$CONTRACT_ID"
$BIN prepare --stage "$STAGE" \
  --compatibility-corpus /home/ian/workspace/repos/pangopup/tests/fixtures/pangolin-compat-v1
$BIN benchmark --stage "$STAGE"
```

Successful preflight creates only the absent private sibling
`.pangopup-mask-stage-$CONTRACT_ID`; prepare seals that same stage; benchmark
seals the report and atomically renames it no-replace to `$OUT/$CONTRACT_ID`.
A handled failure before that ID preserves the deterministic failure-only stage
defined above; later failures preserve the contract stage plus `failure.json`.
There is no automatic retry. Sealed-phase reuse requires a separate canonical
`RUN-READY-REUSE` authorization naming distinct coordinator and reviewer and
the exact prior receipt identities.

## Adversarial Code Review

Reviewer: `/root/ticket012_code_review`

First review verdict: `REJECT`. The reviewer found four Major and three Minor
defects. The compatibility adapter authenticated the corpus and then reopened
`cases.jsonl` by pathname, while the prepare receipt omitted both the corpus and
extracted-point identities. The independent candidate/domain oracle performed
repeated full gene scans. Cancellation did not cover all comparator, reuse,
pre-seal, and pre-publication work. The timed reader carried mutable trace state
and a trace branch, making it non-`Sync`. Receipt maps admitted extra members
and inspection did not reauthenticate every cross-phase identity; a successful
publication rollback ignored failure to sync the parent; and exact GENCODE IDs
were not rejected globally when duplicated across contigs. The reviewer found
no separate major issue outside Ticket 012.

Developer remediation remains inside the accepted contract. Corpus
authentication now returns the exact held `cases.jsonl` bytes plus a
domain-separated aggregate identity; the adapter parses only those bytes and
prepare receipts bind both the exact corpus and the canonical extracted-point
set. The complete-domain proof is now one event sweep plus an indexed domain
oracle, with an operation-count scale control that is more than two orders of
magnitude below the former full-scan lower bound. Candidate construction,
full inspection, comparator work, held reads/copies, reuse, sealing, benchmark
warmup, compression, and publication poll cancellation at bounded points, with
controls for copy, comparator, seal, candidate build/inspection, and
pre-publication interruption. Direct and tracing mmap access are separately
monomorphized over one decoder, so ordinary queries have no trace state or
branch and the reader is compile-time asserted `Sync`. Phase receipts now have
closed exact key sets and inspection checks every source, candidate,
performance, report, and preceding-receipt identity. Rollback parent-sync
failure reports `DURABILITY_UNCERTAIN`, and exact gene identity is globally
unique.

Focused remediation evidence on 2026-07-24: 16 mask lifecycle/oracle tests, 10
candidate-format tests, 9 compatibility unit tests, and 7 compatibility
integration tests passed. Strict feature-gated `pangopup-build` Clippy passed
with warnings denied.

Remediation re-review verdict: `ACCEPT — RUN-READY`. The same reviewer checked
all seven findings against the frozen remediation and found each closed. The
verdict is bound to accepted planning commit `ff5b5e23`, ticket contract
SHA-256 `997abe87caa29fe8f2780d8fd85e317d489d950170eaebe1b189774ba2a1d2c9`,
tracked binary diff SHA-256
`4f317f95d6ed628aed20f5d3845bdee8378b8dcbe3f1d90a57c65de41df719e4`,
and sorted untracked-content manifest SHA-256
`136141c4ff9684d927a3db517fd910abb32ce5961afe8e4cd6626c4cf96ee1e0`.
The exact documented one-time local command is authorized for the coordinator;
no production GENCODE work had run when the verdict was issued.

That `RUN-READY` verdict was invalidated by the first coordinator launch
described below. The reviewed input/launch contract did not provide an
importable gffutils environment, and causal code plus command changes require
the same developer and reviewer to close the defect before another run.

Post-launch remediation is implemented and remains in `review`. The same
developer `/root/ticket012_developer2` owns every code/document change and
performed the integration review. Two implementation children supplied
read-only bounded design analyses—
`/root/ticket012_developer2/python_env_design` for authenticated launch/module
inputs and `/root/ticket012_developer2/preflight_failure_design` for failure
preservation/shell lifecycle. Neither child edited code, documentation, or
generated artifacts, so there is no unreviewed child-authored implementation.

The corrected launch continues to execute the exact held base interpreter but
selects a separately pinned uv venv through an inherited held prefix and
authenticated launcher. It uses `-I -S -B -X pycache_prefix=/dev/null`, inserts
only the held prefix's versioned site-packages directory, and therefore neither
executes venv `.pth` files nor reads existing bytecode caches. The launcher
symlink payload/target relationship, prefix directories, exact `pyvenv.cfg`,
and base interpreter are rebound before and after both helper processes. The
contract and capture receipt bind stable launcher/prefix/base facts, the
`pyvenv.cfg` snapshot, and exact loaded-module evidence. File modules may have
positive hardlink counts (uv's gffutils files have three) but bind and compare
device, inode, link count, size, mtime, ctime, and content hash. Built-in/frozen
modules use an interpreter-owned marker; this correctly represents the pinned
Python's built-in `_sqlite3` instead of inventing a nonexistent shared object.

Pre-final-contract errors now go through the deterministic private failure-only
stage described in the lifecycle contract. The coordinator shell prints the
captured JSON on both success and failure before preserving the original exit
status. Normal offline controls require no local Python: one authenticates a
synthetic launcher/config and selected prefix, one injects missing-gffutils
probe failure and proves a sole canonical private receipt/no scan stage plus
no-overwrite retry refusal, and one proves launcher replacement and hardlink
descriptor drift fail closed. A separate bounded import-only diagnostic (no
database, GTF, annotation scan, capture, or retry) confirmed the pinned local
environment loads gffutils 0.14 and SQLite 3.49.1 with `site` absent,
`sys.pycache_prefix=/dev/null`, and `_sqlite3` origin `built-in`.

Final remediation evidence: 20 focused mask/environment/compatibility tests
passed, the exact SNV regression fixture regenerated byte-for-byte, the
feature-on private binary compiled, and strict feature-on all-target Clippy
passed with warnings denied. Repository-wide `make lint`, `make test`, and
`make spec` are green (`148 passed` at the spec layer). Same-reviewer
re-review verdict: `ACCEPT — RUN-READY`. The reviewer independently recomputed
and bound the corrected candidate to base
`ff5b5e23b9ea6146045c4275de053db605a04727`, ticket SHA-256
`6204bde907ea9de3a00edc1922d26d975e7597f861bbe580e54271b26fd0cf25`,
tracked diff SHA-256
`152c3575bb0be275e91f81ed3354b8dcad37f3e358e25ff4281df48620497b28`,
and sorted untracked-content manifest SHA-256
`01aa8c19633b6b5763738bbffcac9634e6ada81d3150d4a7b58c00afcdb5b79b`.
This fresh verdict supersedes the invalidated historical verdict and authorizes
only the exact corrected capture/prepare/benchmark shell above. No second
production launch had occurred when it was issued.

That verdict became historical when the second launch exposed the
`sqlite3.Row` defect recorded below. Following the bounded row/digest
remediation, the same reviewer first rejected one Minor current-state wording
drift and then accepted the exact two-sentence correction. Final round-two
verdict: `ACCEPT — RUN-READY`, bound to base
`ff5b5e23b9ea6146045c4275de053db605a04727`, ticket SHA-256
`b63a0a744b105084827a9b4f7bcb6fc0af2dc13938bf157d001ae01b8f7e67cf`,
tracked binary diff SHA-256
`7070f7c80155bee321026fa1126cfe62fae20dae40627a55d697df20726a4f6e`,
and sorted untracked-content manifest SHA-256
`19c20effc8dc133a87e45d7d6bb652732b36a7d3b19f846e688c4b2fdc81e8c5`.
The reviewer independently reproduced the exact environment-only v3 probe,
the `2b51bd95...` row control, `99a2bb9a...` schema digest,
`seqidstartend` plan, and 254-module inventory; rechecked the preserved failure
tree; and confirmed all seven original findings plus both launch remediations
closed. This verdict authorizes only the documented third-launch shell for
that frozen tuple, with no reuse or alternate command.

As already diagnosed by the reviewer, the legacy repository-wide SNV builder
fingerprint also observes this unrelated mask Rust change. The source-derived
miniature has therefore been mechanically regenerated after each remediation;
the current builder source SHA-256 is
`58a79c95d90d7f8f4f580d0f5f31c5b34c17ada3619a8cf3df78b2ebee7fdc22`
and fixture bundle ID is
`7f167a07df2dc42be63e6fe2985c3b5d1d469f4977f65d5c765b8756a93a7b32`.
The generated tree matches byte-for-byte and only provenance IDs changed;
source rows, requests, scores, and logical/member identities did not.

The reviewer separately confirmed the already-recorded legacy repository-wide
SNV builder fingerprint as a real provenance/maintainability issue: unrelated
Rust sources churn the miniature SNV bundle identity. It does not block this
qualification, and Ticket 012 does not rebuild any production SNV asset or
change that historical policy.

## Production Job Checkpoint

Purpose: perform the single authenticated local capture, exhaustive candidate
preparation/certification, and retained optimized benchmark authorized by the
`RUN-READY` verdict above. Expected duration is unknown; no automatic retry is
authorized.

Candidate identity for the third launch is the final round-two
ticket/diff/untracked tuple above, helper size 11,397 bytes and SHA-256
`6760e0e3cd706193e65a4669c6c4e842cd30eea096df9d5b75a451422ec9568d`,
the exact database/GTF/Python identities recorded under Implementation
Evidence, Rust 1.93.1, Linux 6.17.0-35-generic, 4,096-byte logical pages, and
inherited CPU affinity mask `0-15`. Immediately before third-launch evidence
transcription, the output filesystem had 152,731,197,440 bytes available. The
exact command is the capture/prepare/benchmark shell block already recorded
above.

Output root:
`/home/ian/workspace/data/pangopup-mask-qualification-012` (confirmed mode
0700 and owner 1000:1000; it was empty before the second authorization and now
contains only the immutable second-launch failure-only stage). Progress is the
appearance and validation of the
contract-addressed stage plus its capture, prepare, and benchmark receipts.
Success is one atomically published final contract directory with a sealed
selection report. Failure is a nonzero command or a sanitized `failure.json`;
the complete private stage must remain preserved and is not resumed or retried
without a new explicit reuse authorization. Safe cancellation is SIGINT to the
foreground qualification process; the implementation kills/reaps its helper,
polls long loops, and preserves the stage receipt.

Coordinator launch: `2026-07-24T21:51:07-04:00`; execution-tool session
`34469`; supervising shell PID `733931`. The release build is the first active
step.

Result: stopped during capture preflight at approximately
`2026-07-24T21:51:30-04:00`, exit status 2. The reviewed release build
completed, but capture created no contract-addressed stage, receipt, snapshot,
observation, candidate, or benchmark output. The output parent exists and is
empty. No production annotation scan occurred.

Causal diagnosis: the exact pinned executable
`/home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13`
runs in isolated mode and cannot import `gffutils` (`ModuleNotFoundError`). The
installed Pangolin tool environment does contain gffutils 0.14 and SQLite
3.49.1, but the reviewed command/code neither identifies nor authenticates that
environment as a launch input. In addition, command substitution retained the
private tool's structured failure JSON without printing it when the command
failed, and pre-contract failures currently leave no durable sanitized receipt.

No retry is authorized. Remediation must add a reproducibly authenticated
Python environment/launcher input, preserve pre-contract failure evidence, and
make the shell surface structured failure output. A new frozen candidate and
same-reviewer `RUN-READY` verdict are required before another absent-stage
launch.

Those requirements were closed by the corrected candidate and fresh
`RUN-READY` verdict recorded above.

Second coordinator launch: `2026-07-24T22:44:22-04:00`. The corrected release
binary again stopped during the bounded environment preflight with exit status
2, before any annotation scan or contract-addressed stage. This time the
failure-only lifecycle worked: the sole new output is private stage
`.pangopup-mask-preflight-failure-81aa7c61319478c77535b731b40c2ca4837e59b6c1b7986d0c2c97c593d6c67d`
containing mode-0400 `failure.json` with SHA-256
`43fc11be4b5dca9b53c37dc70c2c69c645c229ba59ba8949607b62ee4016a62a`.
It records code `PYTHON`, message `environment probe failed`, the exact
preflight contract, and no sealed phases. There is no snapshot, observation,
candidate, prepare receipt, or benchmark output.

A read-only reproduction of only the exact held-descriptor environment helper
(not capture and not a full scan) exposed the first causal operation: gffutils
sets `db.conn.row_factory` to `sqlite3.Row`, so
`list(db.conn.execute("SELECT type,name,tbl_name,sql FROM sqlite_master ..."))`
produces `Row` objects. Python's `json.dumps` rejects those objects before the
helper emits its environment payload (`TypeError: Object of type Row is not
JSON serializable`). The earlier import-only diagnostic could not exercise
this database-specific path.

An in-memory diagnostic that changed only those rows to lists completed the
environment probe and exposed the next deterministic mismatch: the helper's
compact-JSON schema serialization hashes to
`66f1a99c02ff3ab5c1b0e9218b32d450aa5db1ad1fa69e8cff9f232848632e14`,
while the recorded exploratory policy expects `99a2bb...`. The latter has now
been reproduced exactly: it is the same sorted four-column rows serialized as
newline-delimited records with pipe-delimited fields, `NULL` represented as an
empty field, and no final newline (1,512 bytes). This is a serialization-policy
drift, not a difference in the database schema. With explicit row conversion,
the bounded probe otherwise reached the expected Python 3.13.5, gffutils 0.14,
sqlite3 module 2.6.0, SQLite 3.49.1, `seqidstartend` query plan, and 254-module
inventory.

The second `RUN-READY` verdict is therefore invalidated. No third launch is
authorized. The same developer must make the schema-row normalization and
digest serialization explicit, preserve or deliberately replace the now-
reproduced schema identity, and add focused database-row-factory/digest
controls; the same reviewer
must accept the frozen remediation and an exact replacement command/identity
before the coordinator may launch again. The preserved failure stage must not
be removed or overwritten.

### Second-launch developer remediation

The same developer `/root/ticket012_developer2` owns this bounded causal
remediation. Read-only child
`/root/ticket012_developer2/row_normalization_audit` independently inventoried
every SQLite cursor crossing and supplied no implementation edits. The helper
now requires actual `sqlite3.Row` records and converts them positionally with
exact column types for `sqlite_master`, `EXPLAIN QUERY PLAN`, and
`PRAGMA compile_options`. Its in-process control uses duplicate aliases to
prove that `[7,8,null]` survives; treating Row as a mapping would discard the
second duplicate. Observation and capture-contract schemas advance to v3 and
bind the control digest
`2b51bd95640bf4a70aa7a8d44110b390b458a76aefb83458747b051cfd3eba3c`.
The frozen embedded helper is 11,397 bytes with SHA-256
`6760e0e3cd706193e65a4669c6c4e842cd30eea096df9d5b75a451422ec9568d`.

The production schema digest deliberately remains
`99a2bb9a60b4f425dcbf0a497355ea9a204a6d38b9abf69e714db3ef252f7a49`.
It is now computed by an explicitly named legacy transformation: four fixed
positional string/NULL columns, NULL as empty, pipe between fields, LF between
rows, no final LF. Empty strings and pipes are rejected, while SQL-internal
newlines are retained because the pinned schema contains multiline CREATE
statements. This is a secondary observation only; the exact 380,366,848-byte
database SHA-256 remains authoritative. Compact positional JSON is explicitly
not this contract: it is 1,714 bytes and hashes to `66f1a99c...`, rather than
the legacy 1,512 bytes and `99a2bb9a...`.

Unhandled helper exceptions now pass through a path-free exception-class
marker. The Rust boundary accepts only one exact bounded ASCII marker and
records stable `PYTHON_EXCEPTION`, `PYTHON_PROCESS`, `PYTHON_STDERR`, or
`PYTHON_OUTPUT` classifications; it never retains stderr or traceback text in
the failure receipt. Hostile or compound diagnostics collapse to the generic
process classification.

The exact embedded helper was exercised through held base-interpreter, venv
prefix, and database descriptors in environment mode only. This command
creates no stage, snapshot, observation file, or receipt and does not enter the
all-gene scan:

```bash
set -euo pipefail
cd /home/ian/workspace/repos/pangopup
HELPER="$(perl -0777 -ne \
  'if (/pub const OBSERVATION_HELPER: &str = r#"(.*?)"#;/s) { print $1 }' \
  crates/pangopup-build/src/mask.rs)"
bash -c '
set -euo pipefail
helper=$1; python=$2; prefix=$3; database=$4; launcher=$5; base_prefix=$6
exec {python_fd}<"$python"
exec {prefix_fd}<"$prefix"
exec {database_fd}<"$database"
held_prefix="/proc/self/fd/$prefix_fd"
held_launcher="$held_prefix/bin/python"
export __PYVENV_LAUNCHER__="$held_launcher" PYTHONDONTWRITEBYTECODE=1
export OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 NUMEXPR_NUM_THREADS=1
cd /
exec -a "$held_launcher" "/proc/self/fd/$python_fd" \
  -I -S -B -X pycache_prefix=/dev/null -c "$helper" \
  "/proc/self/fd/$database_fd" /dev/null environment \
  "$launcher" "$prefix" "$base_prefix" "$held_prefix" "$python"
' diagnostic "$HELPER" \
  /home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13 \
  /home/ian/.local/share/uv/tools/pangolin \
  /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db \
  /home/ian/.local/share/uv/tools/pangolin/bin/python \
  /home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu \
| jq -c '{kind,schema,python,gffutils,sqlite3_module,sqlite_library,
  sql_row_control_sha256,schema_sha256,query_shape,query_plan,
  module_count:(.modules|length),required_modules:[.modules[].name |
  select(. == "gffutils" or . == "sqlite3" or . == "_sqlite3")]}'
```

It exited zero and returned observation v3, Python 3.13.5, gffutils 0.14,
sqlite3 module 2.6.0, SQLite 3.49.1, the exact row-control and `99a2bb9a...`
schema digests, plan
`SEARCH features USING INDEX seqidstartend (seqid=?)`, 254 authenticated
modules, and all three required module names. The sole preserved production
member remains the second-launch failure receipt with unchanged SHA-256
`43fc11be4b5dca9b53c37dc70c2c69c645c229ba59ba8949607b62ee4016a62a`.

Final developer evidence is green: 22 focused
mask/environment/compatibility tests, including the row/digest and
sanitized-classification controls; strict all-feature, all-target Clippy with
warnings denied; byte-exact regeneration of the 1,000-request SNV fixture;
repository-wide `make lint` and `make test`; and `make spec` (`148 passed`).
The latest unavoidable global builder-fingerprint update changed only builder
source SHA-256 to
`58a79c95d90d7f8f4f580d0f5f31c5b34c17ada3619a8cf3df78b2ebee7fdc22`
and fixture bundle ID to
`7f167a07df2dc42be63e6fe2985c3b5d1d469f4977f65d5c765b8756a93a7b32`;
fresh and checked fixture trees are otherwise byte-identical. The frozen
identity and same-reviewer `ACCEPT — RUN-READY` verdict are recorded above. No
third production launch had occurred when that verdict was issued; no commit
or push has occurred.

Third coordinator launch: `2026-07-24T23:13:08-04:00`; execution-tool session
`44370`. The reviewed release build completed, then capture stopped during the
bounded environment preflight with exit status 2 and canonical error code
`RESOURCE`: `observation helper output exceeds its byte bound`. No annotation
scan, source snapshot, contract-addressed stage, observation, candidate,
prepare receipt, or benchmark ran.

The failure-only lifecycle preserved a second immutable private sibling,
`.pangopup-mask-preflight-failure-f4c44aee64a47e0bc4e495cfac0e9ebf27ee2f50b564a83c968e416a81a8356b`.
Its sole mode-0400 `failure.json` has SHA-256
`ff152f0124384eb2a1969a911f57053290b17d0a5b235abb9eb84cb7200d5d12`
and binds the exact reviewed helper/preflight contract, code `RESOURCE`, no
paths, and no sealed phases. The earlier `81aa...` failure sibling and its
`43fc...` receipt remain unchanged.

A read-only repetition of only the exact environment-mode helper, piped to a
byte counter rather than capture, measured one canonical output line at 79,641
bytes. The implementation applies the shared 65,536-byte
`MAX_METADATA_BYTES` cap to this complete 254-module environment payload, so
the payload that review semantically validated could never pass the capture
or later `environment.json` boundary. This is the first causal divergence;
the full module inventory is required evidence and must not be truncated or
omitted merely to fit the generic manifest/report limit.

The third `RUN-READY` verdict is invalidated. No fourth launch is authorized.
The same developer must introduce a separate explicit bound for complete
environment evidence, keep the 64 KiB manifest/report contracts unchanged,
exercise both sides of the environment-specific bound, and audit every
environment read/write boundary. The same reviewer must bind a replacement
candidate and exact command after independently checking the measured payload.
Both preserved failure siblings must remain untouched.

Third-launch remediation candidate (developer, pending same-reviewer re-review):
complete environment evidence now has its own formal accepted-schema bound.
The helper and Rust validator both require no more than 512 modules, no more
than 512 canonical bytes for each `ModuleIdentity`, and no more than 65,536
canonical bytes for the environment with `modules` removed. The resulting
complete-environment cap is 327,680 bytes (320 KiB). The containing capture
contract separately permits that environment plus an unchanged 65,536-byte
contract envelope, for 393,216 bytes (384 KiB). Generic receipts, inventories,
reports, reuse authorizations, and other metadata remain capped at 65,536
bytes; the full observation stream retains its independent 4 MiB per-line and
1 GiB total bounds.

Every causal boundary uses the distinct limits: environment-only helper drain,
environment canonicalization/staging, full-observation exact comparison,
contract generation and authentication, capture-member authentication,
inspection, and authorized reuse. Final contract derivation and its resource
validation now occur inside the preflight preservation result, before any
contract-addressed stage can be created. A synthetic injected final-contract
resource failure proves that the only output is the deterministic private
`failure.json`, with no contract, source, capture directory, or sealed phase.
The Python helper rejects module 513 before authenticating or hashing it,
rejects a file larger than the already pinned 128 MiB module/source bound from
`fstat` before reading it, and enforces the same per-module, non-module, and
complete-environment canonical limits before emission. Thus module names,
paths, query-plan strings, and other environment strings are subordinate to a
finite accepted-schema bound rather than merely observed-size extrapolation.

Normal synthetic controls exercise environment drain at 327,679, 327,680, and
327,681 bytes; a module at 512 and 513 canonical bytes; a non-module envelope
at 65,536 and 65,537 bytes; rejection of a complete payload over 320 KiB while
every component remains individually legal; and a representative 254-module
environment over 64 KiB. That representative environment passes the complete
`authenticate -> inspect -> preserved post-capture failure -> authorized reuse
-> inspect` lifecycle. Its environment and final contract are both larger than
the old metadata cap, while the capture receipt remains below it.

One ignored, review-only unit test invokes the real private Rust preflight path
directly with the exact pinned database, GTF, base interpreter, and venv
launcher. It cannot create its deliberately nonexistent output parent and does
not invoke full observation mode. The exact command is:

```bash
PANGOPUP_REVIEW_MASK_DATABASE=/home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db \
PANGOPUP_REVIEW_MASK_GTF=/home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.gtf.gz \
PANGOPUP_REVIEW_MASK_PYTHON=/home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13 \
PANGOPUP_REVIEW_MASK_PYTHON_LAUNCHER=/home/ian/.local/share/uv/tools/pangolin/bin/python \
cargo test --locked --package pangopup-build --features mask-qualification \
  mask::tests::review_only_pinned_environment_probe_crosses_rust_bound_without_a_stage \
  -- --ignored --exact --nocapture
```

It passes the exact 79,641-byte, 254-module environment through the Rust drain,
canonical parser, schema/control/plan validation, and final contract derivation.
The canonical final contract is 80,783 bytes with candidate ID
`2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab`.
The declared output parent remains absent and no production capture, annotation
scan, snapshot, candidate, or benchmark ran. Before and after this control, the
production output parent contains only the same two mode-0700 failure siblings;
their mode-0400 receipt hashes remain exactly
`43fc11be4b5dca9b53c37dc70c2c69c645c229ba59ba8949607b62ee4016a62a`
and `ff152f0124384eb2a1969a911f57053290b17d0a5b235abb9eb84cb7200d5d12`.

The frozen embedded helper is now 12,826 bytes with SHA-256
`950ec5d89b57c4e5ad39f621053182acdcbb0c4cf1407abe8455504688d4a8d3`.
The global builder fingerprint is
`5494fc078f55741ef454d64af4fcff47508f2f26be771da818cfc54259f8770d`
and the mask-local fingerprint is
`5f248285fdccb613142a504ea172d7de4f61ea0cf92acd9ffcb0c0bc29c37970`.
The mechanically regenerated 1,000-request SNV fixture changes only that
global builder fingerprint and its derived bundle ID, now
`81c4e331e8461f325175b811159276ae8f090f83ba3c36f5add71b149349cd19`;
normalizing those two identifiers makes the old and new trees byte-identical,
and the checked byte-exact regeneration test passes. Focused mask tests are
green (24 passed, one review-only test ignored), strict all-workspace,
all-feature, all-target Clippy is green, and `make lint`, `make test`, and
`make spec` are green (`148 passed`). The feature-gated release binary builds
and its bounded JSON help contract is intact.

Final same-reviewer remediation verdict: `ACCEPT — RUN-READY`, with no
current-ticket findings. The reviewer independently recomputed and bound base
`ff5b5e23`, ticket `bdededc`, tracked diff `e3baf34a`, untracked tree
`a050e7fe`, and status map `cae00ac2`; audited every distinct cap/read/write
boundary; reran the exact ignored no-stage preflight; confirmed the 79,641-byte
environment, 254 modules, 80,783-byte contract, and `2f6f5bf...` contract ID;
and rechecked that the production root still contains exactly the two
immutable failure-only siblings with unchanged receipt hashes. Earlier
two-launch wording is retained as chronological evidence, not current state.
Authorization applies only to the exact documented fourth-launch shell for
this frozen tuple, with no alternate command or reuse. The known global SNV
builder-fingerprint coupling remains a separate nonblocking issue already
tracked outside this ticket.

Fourth-launch coordinator checkpoint: the reviewer-bound causal tuple above is
unchanged; the following ticket-only verdict transcription is non-causal. At
`2026-07-24T23:53:22-04:00`, expected final stage
`.pangopup-mask-stage-2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab`
was absent, the private output parent contained exactly the two immutable
failure-only siblings above, the filesystem had 151,668,699,136 bytes
available, and inherited CPU affinity was `0-15`. The exact command remains the
documented capture/prepare/benchmark shell; expected duration is unknown.
Progress is sealed capture, prepare, and benchmark receipts for that exact
stage. Success is no-replace publication of the same contract ID; failure is a
nonzero command or sanitized failure receipt. Safe cancellation is SIGINT to
the foreground qualification process. Launch identifiers and current result
follow.

Fourth coordinator launch began at `2026-07-24T23:54:01-04:00` in execution
session `37338`, supervising shell PID `1632783`. Capture created only the
expected private `2f6f5bf...` stage, wrote the exact 80,783-byte contract and
held source snapshots, and entered the full observation helper. Initial
`observation.jsonl` contains only the 79,641-byte environment line; no phase is
claimed sealed until its receipt appears.

Fourth-launch result: capture completed and sealed at
`2026-07-25T00:05:23-04:00`. The immutable 22,947,218-byte observation contains
60,649 genes and 88,202 indexed domains and has SHA-256
`b1dcda39ecb9960a282ba74bb7147dd3536d724ece87fa2cb581afa4ed7b6cac`.
The 79,641-byte environment has SHA-256
`3f2a7837571b0cfc0827ad11a9f313b6ecfdbc358e3b9868e3ede748ccaa2fed`.
The mode-0400 capture receipt is 1,293 bytes with SHA-256
`affcace4fbca05a1d000a3ea9c283fb4d1953aaf469439931f5e37a9c8193238`
and names `prepare` as the next phase.

Prepare then failed closed at `2026-07-25T00:05:24-04:00` before candidate
construction or benchmarking. The mode-0400 failure receipt is 275 bytes with
SHA-256
`ac9411d38ad4cfa710e19f485b5698ba284fb60012dad2e009cd472a3b5b6dcf`;
it records `GTF attribute quoting is invalid`, `failed_phase: prepare`, and
`sealed_phases: [capture]`. The first causal gap is now known: the pinned
official GENCODE v38 GTF uses unquoted decimal attributes such as `level 2`
and `exon_number 1`, while the preparer currently requires every value to be
quoted. The sealed capture and all held inputs remain untouched in the failed
stage. No automatic retry or reuse is authorized. The bounded next action is
to characterize the source's attribute grammar, correct and test the strict
parser, obtain the same independent reviewer's explicit sealed-capture reuse
authorization, and only then reuse the authenticated capture into a new stage
for prepare and benchmark.

Fourth-launch parser remediation candidate (developer, pending independent
review): one complete read-only stream of the exact pinned 46,556,621-byte GTF
(`22020df0...c050`) inventoried all 3,150,429 physical rows: five comments and
3,150,424 valid nine-column data rows. All data rows have a final semicolon and
exactly one ASCII space between each attribute key and value. The stream has
50,091,509 attributes: 44,088,441 quoted and 6,003,068 unquoted. Only two keys
are ever unquoted:

- `level` occurs on every data row: 187,453 values are `1`, 2,770,641 are `2`,
  and 192,330 are `3`;
- `exon_number` occurs 2,852,644 times and its exact distinct set is every
  integer from 1 through 363.

Every bare value is canonical positive ASCII decimal. There is no zero,
leading zero, sign, negative, float, exponent, nondecimal token, whitespace,
quote, empty value, or trailing garbage. Every other key, including all
3,150,424 `gene_id` values and all 7,557,511 `tag` values, is quoted. No empty
segment, bad key, missing value, unmatched/embedded/escaped quote, or internal
semicolon was found. Observed maxima are 25 attributes per row, 24 key bytes,
37 raw value bytes (35 decoded), ten repetitions of one key, and a 685-byte
line. The official GENCODE format documents `level` as 1/2/3 and
`exon_number` as an integer and shows both unquoted:
<https://www.gencodegenes.org/pages/data_format.html>.

The replacement parser therefore retains a closed grammar rather than
accepting generic GTF values. It requires a final semicolon; bounded nonempty
keys and values; exact closed nonempty quotes for string values; and permits a
bare value only for `level` or `exon_number` when it is a nonzero `u32` in
canonical decimal form, with `level` restricted to 1, 2, or 3. Repeated quoted
tags remain ordered values. Arbitrary bare strings, empty quoted strings,
missing/extra/interior quotes, leading zero, zero, sign, float, malformed key,
empty segment, trailing garbage, more than 256 attributes, a key over 64
bytes, or a value over 4 KiB fail closed. These caps comfortably contain every
observed source maximum and remain below the existing 4 MiB line bound.

The causal parser and test change necessarily changes the mask-local builder
fingerprint. Ordinary `reuse` remains intentionally strict and rejects the old
contract; its builder requirement was not weakened or special-cased. A new,
separate `plan-capture-promotion` / `promote-capture` boundary handles only
this cross-builder sealed-capture case. The read-only planner:

1. receives the old builder digest explicitly rather than trusting a stage
   pathname;
2. canonically parses and authenticates the old contract, exact capture receipt
   maps, current helper and environment contract, every receipt-bound source
   and capture member, and the failure receipt;
3. requires `failed_phase: prepare`, exactly `[capture]` sealed, and no prepare
   or benchmark receipt; and
4. derives a target contract by replacing only builder provenance, with an
   explicit structural equality check on every other field.

Promotion then requires one canonical closed authorization with distinct
coordinator and reviewer. It binds both builders, both contracts, the capture
receipt, failure receipt, and exact `[capture]` prefix. It reauthenticates all
source evidence after copying, creates only an absent mode-0700 no-replace
target, writes a current-builder capture receipt binding the authorization and
old receipt, and requires ordinary current `inspect` to pass. It never copies
the old failure receipt, a partial `prepare/` directory, or any candidate. A
handled post-creation cancellation preserves a target-local failure receipt;
the source contract/capture/failure identities remain unchanged.

The exact read-only production derivation command is:

```bash
cd /home/ian/workspace/repos/pangopup
target/release/pangopup-mask-candidates plan-capture-promotion \
  --prior-stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab \
  --source-builder-sha256 5f248285fdccb613142a504ea172d7de4f61ea0cf92acd9ffcb0c0bc29c37970
```

It performs no write and returned the old 80,783-byte contract
`2f6f5bf...fcab`, 1,293-byte capture receipt `affcace4...238`, 275-byte failure
receipt `ac9411d3...dcf`, old builder `5f248285...970`, current builder
`fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816`,
and derived 80,783-byte target contract
`ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb`.
The production source stage was unchanged and that target was absent after the
control.

If and only if the independent reviewer accepts this frozen candidate, the
reviewer must issue the following exact canonical record (one compact JCS
object plus LF) at
`/home/ian/workspace/data/pangopup-mask-qualification-012/capture-promotion-2f6f5bf-to-ce035636.json`:

```json
{"capture_receipt":{"bytes":1293,"sha256":"affcace4fbca05a1d000a3ea9c283fb4d1953aaf469439931f5e37a9c8193238"},"coordinator":"/root","decision":"RUN-READY-CAPTURE-PROMOTION","failure_receipt":{"bytes":275,"sha256":"ac9411d38ad4cfa710e19f485b5698ba284fb60012dad2e009cd472a3b5b6dcf"},"reviewer":"/root/ticket012_code_review","schema":"pangopup-mask-capture-promotion-authorization-v1","sealed_phases":["capture"],"source_builder_source_sha256":"5f248285fdccb613142a504ea172d7de4f61ea0cf92acd9ffcb0c0bc29c37970","source_contract":{"bytes":80783,"sha256":"2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab"},"target_builder_source_sha256":"fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816","target_contract":{"bytes":80783,"sha256":"ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb"}}
```

The proposed coordinator command, not yet authorized or run, is exactly:

```bash
cd /home/ian/workspace/repos/pangopup
target/release/pangopup-mask-candidates promote-capture \
  --prior-stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab \
  --output-parent /home/ian/workspace/data/pangopup-mask-qualification-012 \
  --authorization /home/ian/workspace/data/pangopup-mask-qualification-012/capture-promotion-2f6f5bf-to-ce035636.json
```

Current developer controls are green: the full mask unit surface reports 29
passed and one deliberately ignored production-environment review control;
strict all-workspace/all-feature/all-target Clippy passes. Promotion attacks
cover changed authorization fields and schema, old contract, builder/helper,
receipt, sealed member, failure phase, unsealed partial output, malformed stage
suffix, target collision, cancellation preservation, and source immutability.
The positive synthetic lifecycle proves ordinary reuse rejects the old builder,
promotion changes only builder provenance, current `inspect` accepts the new
receipt, and current `prepare` succeeds. Mechanical SNV fixture regeneration
changed only the unavoidable global builder fingerprint from `5494fc07...70d`
to `fa5d9fc3c3482aeca671e90e75752738019b911c3cba1549bd847856bf3986af`
and derived fixture bundle ID from `81c4e331...19` to
`f7d93978715603eeebb72c7bb1af744e0d3bb5f976c94c3daaeae2c0e6d58fbc`;
normalizing those two identifiers makes the complete old and fresh fixture
trees byte-identical. Repository-wide `make lint`, `make test`, and `make spec`
are green (`150 passed`), as is the stricter all-feature Clippy control. The
candidate is frozen for independent review. No production promotion, prepare,
benchmark, authorization write, commit, push, publication, or Ticket 013 work
has occurred. A post-gate read-only check reconfirmed the source contract,
capture receipt, and failure receipt hashes as `2f6f5bf...fcab`,
`affcace4...238`, and `ac9411d3...dcf`; no `ce035636...` stage or
`capture-promotion-*` authorization file exists in the production parent.

Independent parser/promotion review verdict:
`ACCEPT — RUN-READY-CAPTURE-PROMOTION`. The same code reviewer found no Major
or Minor issue and independently reproduced the complete GTF grammar counts,
official-format agreement, old-stage/capture/failure authentication, the
one-field target derivation, current-builder ordinary-reuse rejection, held
source and no-replace controls, receipt chain, cancellation behavior, full
gates (`150 passed`), and strict all-feature/all-target Clippy. The frozen
causal tuple is HEAD `ff5b5e23`, pre-verdict ticket `1e34d521`, tracked binary
diff `5a9e8e07`, sorted untracked-content manifest `ce8f4f74`, status map
`cae00ac2`, helper `950ec5d8`, current builder `fd738fec`, and target contract
`ce035636`. The source contract/capture/failure identities remained
`2f6f5bf...fcab`, `affcace4...238`, and `ac9411d3...dcf`; the target stage and
authorization were absent. This transcription is non-causal. Authorization is
limited to the exact documented `promote-capture` command and explicitly does
not authorize prepare, benchmark, ordinary reuse, an alternate command, or
publication.

Coordinator promotion result: after independently matching the complete
reviewer-bound tuple, the coordinator wrote the exact 827-byte mode-0400
authorization with SHA-256 `2f864eb2...5fa9` and ran only the authorized
`promote-capture` command. It succeeded without publication and created the
private stage
`.pangopup-mask-stage-ce035636...07fb-promotion-2f864eb2...5fa9`. Ordinary
current inspection reports contract `ce035636...07fb`, `failed: false`, and
exactly `[capture]` sealed. The new 1,467-byte capture receipt has SHA-256
`263dd8ea9466e3019c0b75e907d9c4ed62aaf8fcbf849759ac144994c9c9c057`,
binds current builder `fd738fec...6816`, the authorization, and
`reused_from: affcace4...238`. Observation and environment remain byte-exact at
`b1dcda39...cac` and `3f2a7837...fed`. No failure receipt, prepare directory,
candidate, benchmark, or publication exists in the promoted stage. The old
contract, capture receipt, and failure receipt remained byte-exact. Prepare
and benchmark were not authorized or run; they require a new explicit
reviewer checkpoint over this promoted result.

Post-promotion reviewer verdict: `ACCEPT — RUN-READY-PREPARE-BENCHMARK`, with
no Major or Minor finding. The reviewer independently authenticated the
promoted contract, rewritten receipt, authorization, byte-equal members,
source preservation, compatibility corpus, optimized binary, current builder,
host, resource headroom, and absence of partial/final output. The verdict is
bound to pre-verdict ticket `ab67424f`, tracked diff `8ed38279`, untracked
manifest `ce8f4f74`, status map `cae00ac2`, release binary
`d1770de9892ff0ead5d2d716f4cfc4ea1919077312493de46be5d784cdfa050c`,
builder `fd738fec`, and exact promoted stage above. This non-causal
transcription does not change that tuple. Authorization covers only the exact
documented release-binary `prepare`, a successful read-only `[capture,
prepare]` inspection, and then the exact release-binary `benchmark` on the
same path. It forbids recapture, promotion, reuse, retry, alternate inputs or
binaries, and benchmark after any prepare failure.

Coordinator prepare checkpoint at `2026-07-25T01:01:31-04:00`: inherited CPU
affinity is `0-15`; the benchmark will select CPU0; the reviewer observed CPU0
about 91% idle in its final sample. The host has 22.7 GB available memory and
151,106,265,088 bytes free on the stage filesystem. Progress is a synced
`prepare-receipt.json`; failure is a sanitized `failure.json` with only
`[capture]` sealed. Safe cancellation is one SIGINT/SIGTERM to the foreground
prepare process followed by immediate termination of the sequence. No prepare
or benchmark process existed at this checkpoint.

Prepare result: the exact authorized command exited successfully and sealed
prepare at `2026-07-25T01:02:04-04:00`. It reports 60,649 genes, 88,202
domains, and the exact 1,000-query performance manifest. Current read-only
inspection returns `failed: false` and exactly `[capture, prepare]`. The
1,567-byte prepare receipt has SHA-256 `8fd3b24b...2f0e`; inventory is 647
bytes / `cb4889d0...713b`; performance manifest is 163,320 bytes /
`ffcf6142...1473`. The exact candidate identities are interval-tree
5,763,120 bytes / `c9315500...bd7f`, domains 6,703,320 /
`714b1ac1...d702`, and binned-postings 5,759,360 / `06d20314...25b4`.
There is no failure or benchmark output.

Coordinator benchmark checkpoint at `2026-07-25T01:02:19-04:00`: the required
post-prepare inspection passed, and no benchmark process existed. The exact
reviewer-authorized optimized command and same promoted stage will be used.
Progress is the benchmark report and synced benchmark receipt followed by the
same-parent no-replace final publication. Safe cancellation is one
SIGINT/SIGTERM to the foreground process; any failure ends the sequence with
no retry.

Benchmark result: the exact authorized optimized command exited successfully
at `2026-07-25T01:02:37-04:00`, selected `domains`, sealed benchmark, and
atomically published the stage at
`/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb`.
Ordinary inspection reports `failed: false` and exactly `[capture, prepare,
benchmark]`. The 7,951-byte report has SHA-256 `e064666b...2722`; the
1,013-byte benchmark receipt has SHA-256 `19e4a912...ea10`; capture and prepare
receipt identities remain `263dd8ea...c057` and `8fd3b24b...2f0e`.

All candidates passed semantic, corruption, allocation, and page-trace
controls with zero warmed allocation calls/bytes. Headline p50/p95 were:
`domains` 171/331 ns, `interval-tree` 241/401 ns, and `binned-postings`
241/431 ns. Payload median/p95 pages were 7/9, 8/11, and 6/7 respectively.
Each opened with 1,112 bytes peak Rust heap; measured open times stayed in the
single-digit to low-double-digit microsecond range. Complete member/pinned
Zstandard bytes were `domains` 6,703,320/3,933,486, `interval-tree`
5,763,120/3,554,641, and `binned-postings` 5,759,360/3,393,086. The closed
speed-first selector retained only `domains` at the p95 step. The optimized
run pinned CPU0, recorded zero major faults and 351,670,272 bytes maximum RSS
for the complete benchmark process, and used release binary `d1770de9...50c`.
No retry occurred.

Final developer evidence closeout: a read-only audit authenticated the
published contract leaf, all small reports/receipts, candidate identities, and
the complete receipt chain. The contract, authorization, capture receipt,
prepare receipt, and benchmark receipt identities are respectively
`ce035636...07fb`, `2f864eb2...5fa9`, `263dd8ea...c057`,
`8fd3b24b...2f0e`, and `19e4a912...ea10`. Every downstream reference agrees;
no discrepancy was found. Large source/capture members were reconciled through
their authenticated contract and receipt identities rather than rehashed.

The bounded 7,951-byte report and 163,320-byte JSONL query manifest are now
retained in Git as `planning/artifacts/012-benchmark-report.json` and
`planning/artifacts/012-performance-manifest.jsonl`, byte-exact at
`e064666b...2722` and `ffcf6142...1473`. The human-readable selection evidence
is `planning/artifacts/012-gencode-mask-format-selection.md`; ADR 0011 accepts
only constant-membership domains for later production hardening. README,
architecture, delivery/runtime, FAQ, frontier, and touched documentation-drift
claims now distinguish the selected candidate from the still-missing
production mask format/provider. No database, GTF, observation, canonical
stream, candidate member, or phase receipt was copied into the repository.
No production command, recapture, preparation, benchmark, code change, Ticket
013 draft, commit, or push occurred during this closeout. Focused evidence
checks proved both retained files byte-identical to the published stage,
validated the selected report and all 1,001 JSONL records, and passed
`git diff --check`. The ordinary final gate is green: `make lint`; `make test`
(234 passed, one deliberately ignored production-environment review probe);
and `make spec` (150 passed). Final independent review and coordinator final
check are recorded below.

Final independent reviewer verdict: `ACCEPT — FINAL`, with no Major or Minor
finding. The reviewer independently reauthenticated all 17 published members,
the complete receipt chain, the mechanical `domains` selection, the retained
report and performance manifest, the selector schedule, artifact hygiene, and
the current/future documentation boundary. It also reran the full gate and
strict all-feature Clippy. The accepted pre-verdict tuple binds ticket
`104ed2a0...0c31`, tracked diff `0f31050d...bd4`, untracked manifest
`3f93be2b...ae3`, status map `8a28d1e6...bb88`, selection evidence
`6dba353e...23cd`, ADR `f50438ee...3ba1`, and contract `ce035636...07fb`.
The known repository-wide SNV builder-fingerprint coupling remains a separate,
already-recorded issue and did not affect this bounded selection. This verdict
transcription and the coordinator check below are non-causal record updates;
they do not alter the reviewed implementation or evidence.

## External Effect Evidence

Coordinator: not applicable. This ticket performs local authenticated source
analysis and format selection only. It does not publish, upload, change GitHub
settings, deploy, or replace an active installation.

## Coordinator Final Check

Coordinator `/root`: complete. After independent final acceptance, the
coordinator ran `make lint`, `make test` (234 passed, one deliberately ignored
production-environment review probe), `make spec` (150 passed), and strict
workspace/all-feature/all-target Clippy with warnings denied. Retained evidence
was byte-compared with the published stage and rehashed; the JSONL manifest was
validated as one header plus exactly 1,000 queries with ordinals 0 through 999.
`git diff --check`, stale-claim scanning, artifact-hygiene inspection, and the
check that no Ticket 013 exists also passed. Ticket 012 is ready for the
coordinator-owned implementation commit, push, and separate completed-ticket
cleanup commit.
