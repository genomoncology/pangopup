# 012 — Select exact GENCODE mask semantics and a compact mmap format

Status: ready
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
  state, and rejects change before or during capture. Its authenticated helper
  records Python executable/version, exact helper and imported gffutils source
  identities, gffutils version, `_sqlite3` module identity, SQLite library
  version and compile options, exact region query shape, schema/index identity,
  and relevant `EXPLAIN QUERY PLAN` output.
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

The output root must be absent. Work occurs in a uniquely named private,
contract-addressed sibling stage and publishes no-replace only after successful
capture, semantic comparison, measurement, and report sync. Capture/export,
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

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable. This ticket performs local authenticated source
analysis and format selection only. It does not publish, upload, change GitHub
settings, deploy, or replace an active installation.

## Coordinator Final Check

Coordinator: pending
