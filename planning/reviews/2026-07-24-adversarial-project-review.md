# Adversarial project review — 2026-07-24

## Verdict

Pangopup has a strong shipped foundation. The exact SNV lookup, immutable
release, pinned resumable sync, compatibility corpus, and production reference
provider are unusually explicit about trust boundaries, performance evidence,
and what remains future. The normal installed lookup path has no critical
finding and no observed score-corruption defect. Pangopup is standalone: no
Genome or other GenomOncology software is linked, invoked, or required.

The next roadmap item was too broad and in the wrong order. Packaging
checkpoints and a mask together would freeze two independent representations
before the mask's most important semantic contract was complete. Upstream
Pangolin observes gffutils/SQLite point-query order and mutates shared score
arrays between same-strand genes. That SQL query has no `ORDER BY`, so database
bytes alone do not pin its result order. The retained compatibility cases prove
the behavior in examples, not over all GENCODE genes. GENCODE also supplies
versioned and `_PAR_Y` gene IDs that do not fit the current unversioned lookup
type, and Pangolin's `pos-1` query produces the unusual effective membership
domain `(gene_start,gene_end]`.

The review therefore recasts Ticket 012 as one bounded format-selection
outcome: pin the complete GENCODE v38 mask semantics, prove all-gene order,
resolve versioned identity, compare compact mmap candidates fairly, and select
one representation. Production mask hardening and model runtime/weight format
follow separately. Ticket 012 is independently reviewed and `ready` at the end
of this review; no implementation has started.

No large source, model, reference, or score build was rerun. In particular, the
qualified reference bundle remains preserved and must not be rebuilt merely as
review evidence.

## Review baseline

| Item | Observed value |
|---|---|
| Repository | `genomoncology/pangopup` |
| Revision reviewed | `b2a8d21490196930babd4e05055336fb91101708` |
| Branch | `main`, initially clean and equal to `origin/main` |
| Review date | 2026-07-24, America/New_York |
| Host | Linux x86_64, kernel 6.17.0-35-generic |
| Rust/Cargo | 1.93.1 |
| Mustmatch | 0.1.0 |
| Workspace crates | `pangopup-core`, `pangopup-index`, `pangopup-assets`, `pangopup-build`, `pangopup-cli` |
| Live ticket state | none before review; only `planning/tickets/README.md` |
| Historical tickets | sequential 001–011; completed live tickets removed as designed |
| Public release | `snv-grch38-v1`, immutable, exact eight reviewed assets |

Three read-only sub-agents independently reviewed architecture/documentation,
implementation/security/concurrency, and planning/release state. The primary
review reconciled their evidence against source, tests, specs, retained
artifacts, live processes, dependency metadata, GitHub repository settings,
and the public release.

## Product and system model

Pangopup currently does four useful things:

1. Compiles and serves exact published masked Pangolin scores for covered
   GRCh38 SNVs from a 15,033,158,255-byte memory-mapped member.
2. Installs and selects that immutable lookup bundle under Linux XDG data,
   either from supplied transport files or from one exact binary-pinned public
   release.
3. Retains a strict Pangolin 1.0.2 compatibility oracle for future model work,
   without running Python/PyTorch in normal gates.
4. Builds and reads an exact compact RefSeq GRCh38.p14 reference bundle needed
   by future inference.

It does not yet run the neural model, accept general non-SNV requests through a
public API, route between lookup and inference, operate an HTTP server, package
a container, or install a coherent model/reference/mask profile. The README and
architecture generally state this honestly.

The strongest present invariant is:

> Partial, mixed, corrupt, wrong-identity, or untrusted remote bytes do not
> become the active installed SNV bundle; a successful lookup returns the exact
> published gene-specific values with source provenance.

The intended complete service adds three facts for model fallback: exact
checkpoints, exact reference windows, and exact GENCODE gene/exon masking data.
Those assets have independent identities but must eventually be selected as
one compatible tuple before the service reports ready.

## Verification performed

The review used proportionate checks and did not turn full-data work into a
routine validator.

- The exact reviewed revision had fresh passing local `make lint`, `make test`,
  and `make spec` evidence; 143 executable specs passed. The matching GitHub
  Actions run was green.
- After the review note, issues, frontier, and independently accepted ready
  Ticket 012 were written, the same complete local gate passed again; 143 specs
  passed. The run created no additional lingering upload-test process.
- GitHub's API reported the public `snv-grch38-v1` release as
  `immutable=true`; all eight names, sizes, and digests matched the checked
  release profile.
- All 51 Markdown files had resolvable local link targets.
- `cargo audit` found no advisory in the current 99-package lockfile.
- `cargo deny check advisories bans sources` passed. No repository license
  allowlist exists, so an unconfigured full `cargo deny check` is not treated
  as a valid license verdict.
- A source-path `cargo install --locked` of `pangopup-cli` succeeded and the
  installed executable reported its help/version correctly.
- `cargo package --locked -p pangopup-cli --allow-dirty --no-verify` failed
  because internal path dependencies have no version requirements. Pangopup
  does not currently claim crates.io publication, so this is a future packaging
  requirement rather than a shipped defect.
- `pangopup-build --help`, `--version`, and nested help were exercised and
  reproduced the stale maintainer-interface finding below.
- The Cargo dependency graph was checked against the documented direction. No
  Genome, UTA, SeqRepo, other GenomOncology program, external database, or
  external service dependency exists. `genomoncology` appears only in the
  public repository/release identity.
- The two lingering release-test processes were inspected through `/proc` and
  source correlation. They were deliberately not killed during this read-only
  audit.
- GitHub settings were read through the API. No settings, release, issue,
  branch, or asset was mutated.
- No production input was downloaded or altered. One bounded read-only probe of
  the already-local exact GENCODE database/GTF inspected schema/query plan and
  counted gene/PAR identities; it wrote no export or candidate. No RefSeq,
  Pangolin model, full index/reference build, benchmark, or qualification ran.

## Strengths worth preserving

### Strength — exact shipped scope and evidence

Shipped versus target behavior is mostly clear. `README.md:3-10` says model
inference and HTTP are not implemented. Specs exercise only real CLI behavior;
there are no forward-written model/HTTP/container specs pretending to pass.
Retained artifacts record exact inputs, digests, commands, resource behavior,
failed attempts, and accepted measurements rather than only headline claims.

### Strength — fast-path design

The SNV reader uses checked byte decoding rather than casting mmap bytes into
Rust structs. It opens one provider, returns all overlapping source-gene
records deterministically, and leaves the multi-gigabyte payload to the OS page
cache. The retained measurements distinguish open, lookup, serialization, and
fresh-process CLI cost. The reference provider similarly performs caller-
buffer reads, zero query allocations, and bounded metadata validation without
touching dense pages during open.

### Strength — installed asset trust boundary

The Linux installer uses descriptor-relative no-follow traversal, private
staging, nonblocking locks, exact receipt/member validation, fsync plus atomic
selection, and crash reconciliation. Pinned sync uses exact URLs/sizes/hashes,
strong ETag/range resume, bounded buffering, explicit HTTPS host/redirect
allowlists, and the same installer. It never asks for an unpinned “latest.”

### Strength — release and compatibility discipline

The public SNV release is immutable and separately attributed. Publication
preparation is distinct from the coordinator-only external effect. The strict
compatibility corpus binds source, twelve checkpoints, reference, annotation,
numeric environment, raw dtypes, signed-zero rendering, rejection categories,
and controlled order-dependent masking while keeping model execution out of
normal CI.

### Strength — development process

The repository keeps one reviewed live ticket, uses distinct ticket reviewer,
developer, and code reviewer roles, records evidence in the ticket, commits the
reviewed-ready instruction before implementation, and removes completed live
tickets after shipment. The current planning directory had no duplicate ID,
dependency cycle, stale live status, or prewritten ticket backlog.

## Prioritized findings

### 1. Design gap — High — complete mask order and gene identity were not defined

Evidence:

- ADR 0008 (`architecture/decisions/0008-strict-upstream-compatibility-profile.md:39-47`)
  requires upstream's observed same-strand order.
- Upstream `pangolin.py:64-80` iterates `gtf.region(...)` into Python
  dictionaries; `pangolin.py:133-154` mutates the shared arrays while iterating
  those dictionaries.
- Artifact 009
  (`planning/artifacts/009-upstream-pangolin-compatibility-corpus.md:128-146`)
  explicitly says the corpus lacks a complete all-gene masking-order
  representation.
- Compatibility cases retain versioned IDs such as
  `ENSG00000141510.18`, while `pangopup-core/src/lib.rs:174-214` accepts only
  unversioned 15-character IDs.
- gffutils 0.14 emits the region query without `ORDER BY` and documents default
  order as unspecified; neither the database hash nor package version alone
  freezes the SQLite runtime/query plan.
- A read-only full-database probe found 44 `_PAR_Y` identifiers and 44 stable-ID
  collisions between chrX and chrY copies.
- That probe observed SQLite runtime 3.49.1, database last-written 3.35.4,
  schema digest `99a2bb9a…f252f7a49`, and index plan
  `SEARCH features USING INDEX seqidstartend (seqid=?)`; these facts explain why
  Ticket 012 must capture the complete environment rather than generalize from
  one workstation query.
- Upstream queries `pos-1` before checking the 1-based span, making effective
  membership `(gene_start,gene_end]`, not ordinary inclusive containment.
- The old frontier deferred the overlap-order issue to inference while asking
  the preceding outcome to package a compact mask.

Impact:

A compact file sorted by identifier or coordinate could return the correct set
of genes but a different order, producing different masked scores. A format
could also include the wrong boundary point, discard a GENCODE version/PAR
suffix, or merge chrX/chrY stable-ID collisions, making model provenance and
`--gene` matching incorrect.

Disposition:

This is now Ticket 012. It must authenticate the exact GTF/database plus the
Python/gffutils/SQLite observation environment, freeze a canonical ordered
export, inventory every distinct effective overlap/order domain, preserve full
versioned/PAR identity, define stable filter disambiguation, and select a
measured compact representation. The existing overlap issue is marked promoted
rather than falsely closed.

### 2. Design gap — High — model packaging preceded runtime and tolerance decisions

Evidence:

- `README.md:107-112` promises a verified Rust-runtime representation.
- `architecture/delivery.md:87-98` describes an archive of exact upstream
  checkpoints.
- Upstream `.v2` files are PyTorch ZIP/pickle checkpoint containers loaded by
  `torch.load`; no Rust tensor runtime is selected.
- Artifact 009 says it does not include a tolerance policy, and ADR 0008 does
  not require every intermediate operation to be bit-identical.

Impact:

Converting weights before choosing the consumer can freeze a wrong tensor
layout or unsafe/deserialization boundary. Fourteen end-to-end scored cases are
valuable but are not a discriminating oracle for every channel order,
dilation, transpose, checkpoint selection, or ensemble mistake.

Disposition:

Model work follows mask selection/hardening. The CPU runtime and safe closed
model representation are chosen together. The outcome must authenticate raw
checkpoints before parsing; bind tensor names/shapes/dtypes/order and hashes;
define numeric and rendering tolerance; and add internal/layer or checkpoint
goldens before declaring a converted model production-ready.

### 3. Design gap — High — no atomic compatible four-asset profile was scheduled

Evidence:

- Current `ActiveProfile`/`ActiveBundle` state identifies one SNV bundle
  (`crates/pangopup-assets/src/local.rs:103-159`).
- The complete service requires independent SNV, model, reference, and mask
  assets (`architecture/delivery.md:87-132`).
- Readiness requires all providers to initialize (`architecture/service.md:29-48`).
- Reference transport/install/publication is not shipped.

Impact:

Four independent “active” pointers can select an incompatible tuple. Partial
upgrades, rollback, offline restart, and readiness cannot be defined truthfully
without one atomically selected compatibility manifest.

Disposition:

The frontier now includes a compatible installed runtime profile before HTTP
readiness. It binds all four identities, preserves independent immutable
stores, atomically selects the tuple, publishes/transports the existing
reference without rebuilding it, and proves rollback, offline restart,
partial-upgrade failure, progress, and cancellation.

### 4. Confirmed defect — Medium — an interrupted test can leave a helper forever

Evidence at review time:

- PID 2824225 had PPID 1, had lived more than a day, and slept in
  `anon_pipe_read` while retaining both ends of the test barrier pipe.
- `crates/pangopup-build/tests/transport.rs:653-660` creates inheritable ordinary
  pipes; the abrupt-death setup is at lines 1659-1718.
- The child can block in the before-parent-death-signal test barrier at
  `crates/pangopup-assets/src/release.rs:1922-1934,1966-1970`.

Impact:

If the outer test runner is cancelled or killed, the inherited writer prevents
EOF and the helper remains. It retains a deleted test executable and file
descriptors. This is test/gate process hygiene, not evidence that a normal
installed lookup is unsafe.

Disposition:

Recorded in `planning/issues/2026-07-24-release-process-lifecycle.md` and made a
blocker before the next public upload. Required proof includes close-on-exec
endpoint ownership, explicit closure, RAII cleanup, and a grandparent-death
regression. No public upload is needed to test the repair.

### 5. Risk — Medium — abrupt upload death does not supervise descendants

Evidence at review time:

- PID 2824319 was an orphaned descriptor-holding fake `gh` descendant. Its
  process-group leader was gone and its sealed memfd executable remained open.
- Orderly interrupt/deadline tests kill the whole group
  (`transport.rs:1543-1655,1722-1767`).
- Abrupt-death coverage (`transport.rs:1659-1718`) checks only the direct child.
- `PR_SET_PDEATHSIG` is set only by the direct pre-exec child
  (`release.rs:1933-1943`).

Impact:

The official pinned `gh api` is not shown here to fork a child, so production
severity is conditional. If it does, abrupt coordinator death can leave that
descendant holding upload input or other descriptors.

Disposition:

The same release-process issue requires descendant coverage and either an
independent watchdog that group-kills after coordinator death or a proved,
enforced no-descendant execution boundary. Existing README wording accurately
claims protection only for the direct request.

### 6. Risk — Medium — ambient-path mmap opens rely on a trust assumption

Evidence:

- `BundleOpen::open` checks entries and later calls `File::open` by name
  (`pangopup-index/src/lib.rs:415-456`).
- `ReferenceBundleOpen::open` has the same check/reopen split
  (`pangopup-index/src/reference.rs:179-205,1271-1282`).
- The documented mmap safety contract requires immutable, untruncated members.
- Installed SNV lookup uses held no-follow members and is not affected by this
  path race.

Impact:

An attacker or concurrent writer with directory mutation rights can substitute
a name between checks. Same-inode truncation after mmap can cause SIGBUS. This
is a risk for explicit development paths and future reference/service opens,
not a demonstrated corruption path in the managed installed SNV store.

Disposition:

Recorded in `planning/issues/2026-07-24-runtime-asset-trust-and-durability.md`.
Future service startup must use descriptor-held, no-follow, read-only members;
ambient path opens must either gain the same enforcement or be labeled trusted
development input.

### 7. Confirmed defect — Medium — maintainer CLI help is stale

Evidence:

- `crates/pangopup-build/src/main.rs:73` advertises only `inspect`,
  `prototype-roundtrip`, `prototype-open`, and `benchmark-corpus`.
- Dispatch at lines 75-167 also implements `build`, `verify`, `reference`,
  `transport`, `release`, `compatibility`, and `reference-candidates`.
- `pangopup-build --help` and `--version` exit 2 with that stale four-command
  usage; nested `--help` is treated as an invalid action.
- Existing specs do not cover the complete builder help surface.

Impact:

Maintainers cannot discover shipped production commands from the tool itself,
and adding mask/model commands would compound the drift.

Disposition:

Recorded with stale documentation in
`planning/issues/2026-07-24-maintainer-interface-and-documentation-drift.md`.
It does not block local mask semantics work, but it must be resolved before new
maintenance commands are presented as supported user-facing interfaces.

### 8. Security risk — Medium — repository controls lag release rigor

Evidence on 2026-07-24:

- GitHub reported no branch protection and no rulesets for `main`.
- Dependabot security updates, secret scanning, non-provider scanning, push
  protection, and validity checks were disabled.
- `.github/workflows/ci.yml:11` uses mutable `actions/checkout@v4`.
- The workflow has no explicit minimum `permissions:` declaration.
- `mustmatch==0.1.0` is version-pinned but installed without a content hash.

Impact:

The publication tool authenticates bytes more strongly than the repository
currently protects changes and credentials. This is not evidence of current
compromise and does not block local format work.

Disposition:

Recorded in `planning/issues/2026-07-24-publication-security-baseline.md` and
made a gate before public model/reference/executable/container release. The
resolution includes full-SHA Action pins, least privilege, available GitHub
security facilities, a status-check policy compatible with coordinator-owned
pushes, and enforced advisory/license/SBOM/provenance work.

### 9. Design gap — Medium — non-SNV identity and normalization are undefined

Evidence:

- The public core has only `Grch38Snv` (`pangopup-core/src/lib.rs:269-311`).
- Future cache text uses “normalized variant”
  (`architecture/service.md:73-84`).
- Pangopup deliberately excludes general HGVS/normalization.

Impact:

Equivalent indel spellings, reference validation, supported Pangolin allele
shapes, provenance, and future cache keys are ambiguous. A model provider cannot
expose a stable typed request until this is resolved.

Disposition:

This blocks the CPU inference/public routing boundary, not Ticket 012. The
future outcome must either require a documented anchored/minimal genomic form
from callers or implement only the narrow normalization needed for supported
Pangolin shapes. It must define equivalence and cache identity explicitly.

### 10. Risk — Medium — provisioning can remain unready indefinitely

Evidence:

- Sync has connect/header/read-idle timeouts but no whole-member or full-profile
  deadline (`pangopup-assets/src/sync.rs:223-245,1014-1048`).
- Service startup may invoke sync (`architecture/service.md:41-48`).
- Persistent progress/status is future (`README.md:271-275`).

Impact:

An allowlisted endpoint that trickles one byte inside each idle timeout can hold
the sync lock and keep a service unready indefinitely. Integrity remains safe;
the failure is liveness and operability.

Disposition:

The compatible-profile outcome must define monotonic overall deadlines or
cancellation, resumable partial state, and durable non-secret progress before
HTTP startup adopts automatic provisioning.

### 11. Durability/maintainability risk — Medium — evidence and source identities are coupled poorly

Evidence:

- The production reference bundle and canonical qualification roots exist only
  under workstation-local paths recorded at
  `planning/artifacts/011-production-reference.md:40-56,251-260`.
- Small canonical reports/receipts are summarized but not portable with the
  public repository.
- `crates/pangopup-build/build.rs:20-54` fingerprints every Rust file in core,
  index, assets, and build for all builder artifacts.
- Artifact 009 records that unrelated compatibility code churned the SNV
  builder identity although `scores.pgi` and NOTICE were byte-identical.

Impact:

Losing the local reference roots would force a needless rebuild and lose
machine-readable evidence. New mask code can also churn unrelated artifact
identities and fixtures.

Disposition:

Ticket 012 requires an artifact-specific mask source inventory. The runtime
asset issue requires preserving existing small reference receipts and later
publishing/transporting the unchanged large member. Existing production
artifacts are not rebuilt just to change fingerprint policy.

### 12. Inconsistency — Low — Ticket 011 and sync left stale claims

Confirmed examples:

- `planning/faq.md:153-170` says no reference runtime exists and production
  hardening remains.
- `architecture/design.md:101-102` says `pangopup-assets` has no network access,
  although it owns `ureq` sync.
- `architecture/design.md:91-94` claims a CLI streaming mode that does not
  exist; the CLI supports repeated batch variants.
- `release-profiles/README.md:20-21` calls shipped remote sync the next slice.
- `planning/artifacts/011-production-reference.md:38` points to an active
  Ticket 011 that was intentionally removed.
- `README.md:430-432` does not mark outcome 11 complete and still combines
  model/mask packaging as outcome 12.

Impact:

The stale-claim check in the Ticket 011 lifecycle did not cover all public and
maintainer documents. The system itself is not affected, but readers receive
contradictory current/future state.

Disposition:

Recorded in the maintainer-interface/documentation issue. Ticket 012 names the
docs it must reconcile where its new boundary changes them; unrelated stale
help/docs remain an explicit maintenance slice.

### 13. Packaging gap — Low now, Medium before executable distribution

Evidence:

- Source-path `cargo install --locked` succeeds.
- `cargo package` rejects internal path-only dependencies without version
  requirements.
- No executable release, crates.io package, or container is currently claimed.

Impact:

The workspace cannot presently be published as ordinary Cargo packages. This
is harmless for source builds today but must not be discovered during release
day.

Disposition:

The executable/release outcome must choose the distribution boundary and test
that exact artifact on a clean machine. Add internal versions if crates.io
publication is chosen; do not change manifests merely to satisfy an unused
channel.

### 14. Performance/maintainability risks — Low — measure before redesign

`ScoreProvider::lookup` returns owned `Vec` results and clones provenance. The
retained benchmark honestly reports eight allocations/222 bytes for one lookup
and 800/22,200 bytes for 100 while latency remains sub-microsecond per lookup.
The future HTTP batch benchmark should measure allocator pressure and add a
visitor/borrowed rendering path only if concurrency makes it material.

Security-critical modules are large and repeat some `openat2`/directory/process
primitives. Do not launch a line-count refactor. When mask/reference/model
stores need the next concrete use, reuse an existing audited primitive or
extract the smallest shared internal boundary so a fourth subtly different
filesystem policy is not created.

## Planning and ticket assessment

Before this review there was no active ticket. Historical ticket IDs were
sequential and their recent reviewed-ready/implementation/code-review records
used distinct roles. No backlog, duplicate requirement, impossible acceptance
criterion, or status disagreement needed repair.

The old current outcome was materially too broad. The new rolling order is:

1. **Ticket 012 (reviewed and ready):** exact GENCODE/gffutils semantics,
   complete order/identity proof, measured compact candidate selection.
2. Harden the selected production mask bundle/provider.
3. Choose the CPU runtime and safe model representation together; add tensor-
   level discriminating evidence and tolerance policy.
4. Implement CPU inference with an explicit supported genomic-allele contract
   and explicit reference/mask/model paths.
5. Implement lookup-first typed routing.
6. Install and atomically activate one compatible four-asset profile, including
   bounded progress/cancellation and offline restart.
7. Measure repeated inference and make an evidence-based cache/no-cache
   decision.
8. Add a bounded foreground HTTP contract.
9. Add non-root Docker and systemd lifecycle proofs as separate slices.
10. Publish executables and complete repository, dependency, SBOM/provenance,
    concurrency, observability, security, upgrade, rollback, and clean-machine
    hardening.

This is an outcome map, not ten prewritten tickets. Only Ticket 012 is a live,
independently reviewed ticket.
Each later ticket is written from the actual preceding result.

## Planning changes made by this review

- Rewrote `planning/frontier.md` so mask semantics/format selection precedes
  production mask hardening and model runtime selection.
- Updated `planning/goals.md` so model fallback requires an explicit genomic-
  allele identity and delivery atomically selects one compatible asset tuple.
- Added an explicit compatible installed runtime-profile outcome before HTTP
  readiness.
- Promoted the remaining complete overlap-order work into Ticket 012 without
  pretending Ticket 009 proved the entire annotation database.
- Drafted and independently reviewed
  `planning/tickets/012-select-gencode-mask-format.md`. The reviewer rejected two
  revisions, closing unordered-query authority, `_PAR_Y` collisions,
  `(start,end]` membership, benchmark fairness, private tooling, source limits,
  and failed-stage preservation before accepting it as `ready`. It performs no
  public external effect and no model work.
- Added bounded issue records for maintainer help/documentation drift,
  release-process lifecycle, publication security, and runtime asset
  trust/durability.
- Did not draft Ticket 013 or a backlog.

## Open questions that do not block Ticket 012

- Which CPU tensor runtime gives the best safe, supportable checkpoint boundary?
- What tolerance is acceptable for final CPU scores and public rendering when
  internal operations are not bit-identical?
- Does the authenticated official `gh api` executable ever create descendants
  on the supported publication path?
- Should executable distribution use GitHub binary releases only, crates.io,
  or both?
- Which repository ruleset preserves the project's coordinator-owned delivery
  process while still requiring green CI?
- Does repeated real model inference justify any application cache?

These decisions have named later gates. None justifies mixing model conversion,
HTTP, Docker, or public publication into Ticket 012.

## Final recommendation

Proceed, but with the corrected boundary. Ticket 012's independent review is
recorded; start Ticket 012 next after this planning-only change passes its gate
and is pushed. Do not start model packaging, do not rebuild the reference, and
do not publish another asset until the corresponding recorded blockers are
closed.
