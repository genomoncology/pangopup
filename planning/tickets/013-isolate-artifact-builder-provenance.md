# 013 — Isolate SNV and reference builder provenance

Status: ready
Accepted v1 contract identity:
`4bef0284bcaa04dad90bc438a95db668e07c4c1c66007ab3980acdc720ced0c9`
Replacement remediation contract identity
(SHA-256 of this ticket with this value set to `pending`): `27904a7344675044143405c4b7784a1dd33ce070e9526f4437befe70df2fc3d1`
Base revision: `c5ec1a3f07d8c72b71dc76b895f3784a48a81a5d`
Reviewed-ready implementation base:
`17681a2b6807333504cc2188626ded8578f90019`

## Why

The SNV and production-reference builders currently write the same
repository-wide source fingerprint into their manifests:
`fa5d9fc3c3482aeca671e90e75752738019b911c3cba1549bd847856bf3986af`.
`crates/pangopup-build/build.rs` obtains that value by hashing 46 files,
including mask qualification, candidate, transport, sync, release, and test
code that cannot change either produced payload.

Ticket 012 consequently changed the checked SNV bundle identity even though
`scores.pgi` and `NOTICE` remained byte-identical. Production mask work would
repeat that churn unless the older artifact families first own separate
provenance.

This ticket gives future SNV and reference builds distinct, versioned,
artifact-local source fingerprints. It proves the migration entirely with
miniature assets. Existing public/qualified assets remain readable and are not
rebuilt, repacked, downloaded, or published.

## Pinned facts and independent baseline

- The exact base is the clean public commit above. There are no user or
  concurrent changes.
- The current shared builder fingerprint is
  `fa5d9fc3c3482aeca671e90e75752738019b911c3cba1549bd847856bf3986af`.
- The Ticket 012 mask fingerprint is independently scoped but includes
  `crates/pangopup-build/build.rs` and other pinned inputs. Its exact accepted
  value is
  `fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816`.
  This ticket must not change it.
- The checked SNV regression bundle currently has:
  - manifest/bundle: 3,370 bytes,
    `f7d93978715603eeebb72c7bb1af744e0d3bb5f976c94c3daaeae2c0e6d58fbc`;
  - `scores.pgi`: 6,560 bytes,
    `fb0a77425456bd39e6aab7ad3447a24757f6889e82f7b27df01c214b78f8a6b9`;
  - `NOTICE`: 1,709 bytes,
    `9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7`.
- A coordinator-built miniature reference baseline from the base revision is
  retained under ignored `target/ticket013-baseline/reference/`:
  - manifest/bundle: 3,333 bytes,
    `9712bea82a93ae97332c47d5ebc6e2bd4a30385315d170416e0ef95a4caac104`;
  - `reference.pgr`: 4,560 bytes,
    `0ef815ffb3fbb897e880e56afcb57e1edb41f3707784f591c0457581c2e9a3d5`;
  - `NOTICE`: 279 bytes,
    `faea3b1976bf4e15f95bad3906144d83b4441f860d3c5b87ab406205e47262db`.
- Existing v1 readers validate a nonempty builder version and a syntactically
  valid SHA-256. They do not require an old bundle's fingerprint to equal the
  current executable. The immutable public SNV release and retained reference
  therefore need compatibility controls, not rewritten artifacts.
- The public SNV release remains pinned to builder `10fd5d77...2b64`, bundle
  `c4c4162b...819b3`, and score member `6fd8eb49...bf27`. The retained
  production reference remains pinned to builder `2215dabe...1d8f`, bundle
  `7c28334e...dc5f`, and member `cdec4b62...a82`.

## Scope

### Included

- Give future SNV builds the domain
  `pangopup.snv-builder-source.v1` and future reference builds the distinct
  domain `pangopup.reference-builder-source.v1`.
- Use one shared length-framed SHA-256 algorithm over a canonical, sorted,
  duplicate-free list of logical source/dependency identities. The domain,
  inventory declaration, logical path, byte length, and bytes all participate
  in the digest.
- Embed the selected source and dependency evidence in the compiled builder.
  Building an artifact must not inspect the checkout, invoke Cargo recursively,
  or depend on mutable source paths at runtime.
- Extract only code that is presently co-located with unrelated subsystems and
  is needed to make honest file-level inventories:
  - move the private SNV index implementation out of
    `crates/pangopup-index/src/lib.rs` while preserving its current root
    re-exports and public API;
  - move SNV source ingestion out of `crates/pangopup-build/src/lib.rs` while
    preserving current re-exports;
  - move the SNV `NOTICE`/bundle-certification implementation out of
    `crates/pangopup-assets/src/lib.rs` while preserving current re-exports.
  - move only the shared build-command error type out of
    `crates/pangopup-build/src/production.rs`, and the SNV bundle error/input
    audit support out of `crates/pangopup-assets/src/lib.rs`, into bounded
    private modules included by every inventory that imports them. Preserve
    existing error behavior and API paths.
- Treat `crates/pangopup-core/src/lib.rs` as a conservative shared causal input
  to both v1 inventories. Do not refactor it in this ticket because it is a
  pinned input to the accepted mask identity.
- The SNV inventory must cover its domain/inventory declaration and hashing
  algorithm, root `NOTICE`, shared core vocabulary, extracted SNV index, SNV
  ingestion, SNV bundle construction/certification, and an artifact-specific
  projection of the exact locked external dependency closure/features used by
  those sources.
- The reference inventory must cover its domain/inventory declaration and
  hashing algorithm, shared core vocabulary, production reference index and
  builder/certifier, the compiled
  `tests/fixtures/pangolin-compat-v1/cases.jsonl` certification oracle, and its
  own exact locked external dependency closure/features.
- Keep dependency projections checked, canonical, and artifact-specific.
  Whole-workspace `Cargo.toml` or `Cargo.lock` bytes are not fingerprint inputs:
  adding an unrelated future dependency must not churn either artifact.
- Prove each direct external root independently from the selected causal Rust
  sources and the actual dependency declaration of the crate that owns each
  source. The proof must derive candidate dependency identifiers and exact
  version/default-feature/feature settings from the relevant real Cargo
  manifests, then compare them with the artifact root declaration. A checked
  witness file cannot be the authority for this comparison. Omitting a root
  and its witness together, or changing a causal manifest dependency setting
  without refreshing the artifact projection, must fail.
- Resolve each accepted root set in its own standalone, locked, offline Cargo
  project so unrelated workspace members cannot unify features into either
  projection. Relevant Cargo manifests and the workspace lock may be test
  evidence, but their whole bytes do not become fingerprint inputs.
- Change only future manifest `builder.source_sha256` values. Keep
  `pangopup.bundle.v1`, `pangopup.reference.bundle.v1`, their builder object
  shape, member formats, package-version field, readers, public APIs, and CLI
  output contracts compatible.
- Preserve an incumbent miniature manifest for each family as a checked legacy
  reader control. Reuse the new byte-identical payload/notice in test scratch;
  do not duplicate a large or production member.
- Regenerate the checked SNV regression fixture once with its established
  deterministic generator. Its six source members, reference, requests,
  `NOTICE`, and `scores.pgi` must remain byte-identical. README, manifest, and
  expected JSONL may change only where their bundle provenance changes.
- Add a bounded evidence note at
  `planning/artifacts/013-artifact-builder-provenance.md`.
- Add ADR 0012 for artifact-specific, domain-separated builder provenance and
  update `architecture/README.md`, `architecture/index.md`,
  `architecture/runtime-data.md`, `README.md`, `planning/frontier.md`, and
  `planning/issues/2026-07-25-artifact-builder-fingerprint-coupling.md`.

### Excluded

- Editing `crates/pangopup-build/build.rs`, any current mask-fingerprint input,
  any Cargo manifest, or `Cargo.lock`.
- Changing the mask qualification identity, promoting/replaying its capture,
  rebuilding candidates, or reading GTF/SQLite production inputs.
- Rebuilding, verifying, repacking, downloading, installing, or publishing the
  complete SNV or reference assets.
- Changing manifest schemas, adding an optional provenance field, changing
  payload formats, or changing runtime lookup/window behavior.
- Reference qualification, performance benchmarking, transport/XDG/release
  work, public maintenance commands, model inference, routing, HTTP, or Docker.
- A general crate/module cleanup. Mechanical extraction is limited to the
  three SNV roots and three bounded shared support modules named above and must
  preserve existing API paths and behavior.

## Success Checklist

- [x] The current defect has a discriminating control: the SNV and reference
      fingerprints differ, and representative mask/candidate/sync/release/CLI
      inputs are excluded from both.
- [x] Pure hasher tests inject source/dependency bytes and prove:
  - mutating every declared SNV-only input changes only SNV;
  - mutating every declared reference-only input changes only reference;
  - mutating shared core or shared fingerprint-algorithm input changes both;
  - changing an excluded opposite-family or unrelated input changes neither;
  - order does not change the digest, duplicates are rejected, and add/remove/
    rename/domain-version changes do;
  - every compiled inventory and dependency projection is canonical, complete
    against its declared map, and independently recomputes the emitted value.
- [ ] An independent dependency control derives direct external uses from the
      selected causal source bytes and binds each one to its actual owning
      Cargo-manifest declaration. Simultaneous root/witness omission and a
      causal mutation of the version requirement, `default-features`, or
      feature list each fail; an unrelated workspace consumer or dependency
      does not change either projection.
- [x] A focused test asserts the exact Ticket 012 mask fingerprint remains
      `fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816`.
- [x] New miniature SNV and reference manifests carry distinct valid
      fingerprints under their respective v1 domains.
- [x] The current reader opens an actual pre-migration miniature SNV manifest
      and the coordinator-pinned pre-migration reference manifest with their
      unchanged members and returns the expected lookup/window.
- [x] SNV fixture migration proves the ten invariant files byte-identical and
      proves every expected JSONL change is limited to
      `provenance.bundle_id`.
- [x] Reference migration proves exact unchanged hashes for `reference.pgr`
      (`0ef815ff...a3d5`) and `NOTICE` (`faea3b19...62db`) while the manifest
      receives reference-specific provenance.
- [x] Existing production release-profile/proof constants and retained
      production identities are unchanged. No production asset is opened.
- [x] The 1,000-case SNV regression, reference miniature tests, full-bundle
      tests, transport/release tests, and mask-qualification miniature tests
      remain green.
- [x] No new public command or spec behavior is introduced. Existing
      `make spec` remains the outside-in regression gate.
- [ ] `make lint`, `make test`, and `make spec` pass.

Focused commands:

```text
cargo test --locked -p pangopup-build source_fingerprint
cargo test --locked -p pangopup-build --test snv_regression_fixture
cargo test --locked -p pangopup-build --test full_bundle
cargo test --locked -p pangopup-build --test reference
cargo test --locked -p pangopup-assets release
cargo test --locked -p pangopup-build mask::tests
```

The exact filtered test names may be adjusted to the accepted module names,
but each stated control must run rather than being satisfied by a filter that
matches zero tests.

## Decisions

### 1. Artifact-local compile-time evidence, not another build-script hash

- Considered: edit the global `build.rs`; generate fingerprints with nested
  Cargo; compute from mutable checkout paths at runtime; embed causal bytes and
  hash them inside the owning builder.
- Decision: embed and hash artifact-local evidence inside the builder.
- Why: `build.rs` is itself a Ticket 012 mask input, nested Cargo is fragile,
  and runtime checkout reads make released binaries non-reproducible.

### 2. File-granular causal inventories with bounded narrow extractions

- Considered: keep hashing mixed crate roots; perform a repository-wide module
  rewrite; extract only the SNV implementation from three mixed roots; extract
  the small shared support definitions needed to close those inventories.
- Decision: perform the three named SNV extractions plus only the shared
  build-command error, asset error, and asset input-audit support extractions
  named in Scope. Keep shared core as an explicitly shared conservative unit.
- Why: this removes the known mask/candidate/delivery coupling without turning
  a provenance correction into a broad API refactor, while ensuring that an
  imported support definition cannot change artifact behavior outside its
  fingerprint. It does not change the accepted mask fingerprint.

### 3. Version the hash preimage, not the v1 manifest schema

- Considered: add `fingerprint_schema` to existing builder objects; create new
  bundle schemas; use distinct domain identifiers inside the existing digest.
- Decision: keep both manifest schemas unchanged and domain-separate the hash
  preimages.
- Why: old readers use closed JSON structs and would reject extra v1 fields.
  The domain identifies the algorithm in source/ADR/evidence while preserving
  asset compatibility.

### 4. Keep legacy assets immutable

- Considered: rebuild production assets under the new identity; special-case
  old digests in readers; retain old miniature manifests and rely on the
  existing syntactic digest contract.
- Decision: use real miniature legacy manifests and leave reader rules and
  production assets unchanged.
- Why: builder provenance is descriptive, not a runtime compatibility key.
  Rebuilding billions of rows would add cost without changing a score byte.

### 5. Dependency projection rather than the whole lockfile

- Considered: omit dependencies; hash the entire lockfile; record the exact
  artifact-specific resolved closure/features; trust a second checked witness
  declaration; derive the roots from selected causal sources and their owning
  package manifests.
- Decision: selected causal Rust source bytes plus the actual dependency
  declarations in their owning Cargo manifests are the authority for direct
  roots, version requirements, `default-features`, and feature lists. Checked
  witness declarations are diagnostic only. Resolve each derived root set in
  a standalone locked project and record its exact closure/features in a
  canonical projection.
- Why: dependency versions can affect construction/certification, while an
  unrelated future model or HTTP dependency must not change SNV/reference
  identities.

### 6. Canonical dependency target

- Considered: use whichever target runs the tests; record every supported
  target; bind the v1 projections to one production target.
- Decision: the two v1 dependency projections are canonically resolved for
  `x86_64-unknown-linux-gnu`, the current production artifact-build target.
  Tests on any host inspect that target explicitly. Another production target
  requires a separately reviewed fingerprint domain/version rather than
  silently changing v1.
- Why: target-specific dependencies and features must not vary with the
  maintainer's test host, and Pangopup currently makes production publication
  guarantees only on Linux.

## Dependencies

None.

## Work Ownership

- Coordinator/ticket author: `/root`.
- Read-only research:
  `/root/ticket013_fingerprint_code_research` and
  `/root/ticket013_evidence_research`. They supplied evidence and cannot serve
  as this ticket's design or code reviewer.
- Independent design reviewer: `/root/ticket013_design_review3`.
- Developer: `/root/ticket013_developer`; differs from the research agents and
  reviewers.
- Independent code reviewer: `/root/model_architecture_research`; differs from
  author, ticket research agents, design reviewer, and developer.
- Pre-existing user changes: none.
- Coordinator-generated ignored baseline:
  `target/ticket013-baseline/reference/`.
- Tracked implementation:
  - narrow SNV extractions in `pangopup-index`, `pangopup-build`, and
    `pangopup-assets`, preserving their root re-exports;
  - bounded shared command-error and asset error/input-audit support
    extractions required to close the causal inventories;
  - `pangopup-build` artifact-local fingerprint algorithm, declarations,
    source inventories, isolated dependency roots/locks/projections, and
    manifest wiring;
  - discriminating fingerprint and migration integration tests.
- Tracked generated evidence:
  `tests/fixtures/builder-provenance-v1/` and the one-time regenerated
  `tests/fixtures/snv-regression/` manifest/README/expected JSONL. Its source,
  reference, requests, notice, and score member remain unchanged.
- Tracked documentation:
  ADR 0012, architecture/runtime/README/frontier/issue updates, and
  `planning/artifacts/013-artifact-builder-provenance.md`.
- Concurrent unrelated work: none.

## Long-Running Jobs

None. This ticket uses only miniature fixtures and normal gates. Production
data, model capture, and performance qualification are forbidden.

## Coordinator Authorship

Coordinator: `/root`

The ticket was authored from commit `c5ec1a3`, the retained Ticket 009/011/012
evidence, current readers/builders, two independent read-only research audits,
and freshly pinned miniature baselines. The coordinator does not implement or
review it.

## Independent Ticket Review

Reviewer: `/root/ticket013_design_review3`

Verdict: `ACCEPTED AS READY`.

The reviewer independently matched the exact proposed-ticket SHA-256
`4bef0284...ced0c9` and base `c5ec1a3...81a5d`, inspected the directly
relevant builders, fingerprint boundary, frontier, and issue, and found no
Major or Minor finding. It confirmed that the contract is feasible without
editing `build.rs` or Cargo inputs, preserves the accepted Ticket 012 mask
digest, keeps both v1 schemas/readers compatible through real miniature legacy
manifests, and uses discriminating provenance and payload-invariance controls.
Dependencies remain `None`. This verdict transcription and status transition
are non-causal record changes after acceptance of the exact identity above.

Replacement remediation review:

- Identity `30022b75...6208` was `REJECTED`. Decision 2 still contradicted the
  bounded support extractions, Decision 5 did not record the new source/
  manifest authority or canonical target, and acceptance did not separately
  discriminate version-requirement and `default-features` drift.
- Identity `27904a73...c3d1` resolved those three findings. The same reviewer
  independently recomputed the normalized identity, found no Major or Minor
  findings, confirmed base `c5ec1a3`, implementation base `17681a2`, and
  dependencies `None`, and returned `ACCEPTED AS READY`. This verdict
  transcription and status transition are non-causal record changes after
  acceptance of the exact replacement identity above.

## Implementation Evidence

Developer: `/root/ticket013_developer`

Implementation completed against reviewed-ready commit `17681a2` and the
accepted contract identity above. The pre-edit checkout was clean; the
coordinator-owned ignored reference baseline matched all three pinned hashes,
and there was no concurrent unrelated work.

The mixed SNV implementations were mechanically extracted into private
`snv.rs` modules while preserving every root re-export. Future SNV and
reference manifests now call separate artifact-local fingerprint functions.
Their shared v1 algorithm length-frames the algorithm declaration, domain,
inventory declaration, and sorted unique logical path/bytes. All evidence is
compiled into the builder.

Checked dependency roots, isolated locks, source-use witnesses, and projections
are independently compared in standalone per-artifact temporary projects with
`cargo metadata --locked --offline --filter-platform
x86_64-unknown-linux-gnu`. No workspace consumer participates in those
resolved feature sets. A missing `libc` root and a broken witness both fail.
Pure injected-input controls exercise every declared family input, shared
inputs, excluded actual paths, order, duplicate, add, remove, rename,
inventory, domain, and algorithm changes. An independent one-shot preimage
oracle and hard expected values check the production incremental hasher. The
current miniature identities are SNV `536ec535...d825a6` and reference
`5eb85f90...3f41f2`.

The established fixture generator was rerun after reviewer-required inventory
closure. Ten SNV inputs/members stayed at their exact accepted hashes; only
`builder.source_sha256` and copied bundle IDs changed. Legacy manifest
rebinding reconstructs exact old canonical bytes.
The new integration controls also build the miniature reference, compare its
member/notice with the coordinator baseline, open both actual legacy manifests,
and return a real lookup/window. Production identities are checked from
unchanged source constants without opening production assets.

An aggregate filtered rerun exposed that the new SNV/reference migration tests
shared one process-named scratch directory. The implementation corrected them
to use family-specific scratch paths. Reviewer remediation subsequently
expanded the unit controls; the final aggregate passes 7 unit plus 4 migration
controls.

Focused evidence:

```text
cargo check --locked --workspace --all-features
  PASS
cargo test --locked -p pangopup-build source_fingerprint -- --nocapture
  PASS after final remediation: 7 unit + 4 integration
cargo test --locked -p pangopup-build --test builder_provenance -- --nocapture
  PASS: 4
cargo test --locked -p pangopup-build --test snv_regression_fixture -- --nocapture
  PASS: 1 (1,000 requests)
cargo test --locked -p pangopup-build --test full_bundle -- --nocapture
  PASS: 13
cargo test --locked -p pangopup-build --test reference -- --nocapture
  PASS: 15
cargo test --locked -p pangopup-assets release -- --nocapture
  PASS: 4 matched controls
cargo test --locked -p pangopup-build --features mask-qualification mask::tests -- --nocapture
  PASS: 29; 1 intentional production lifecycle control ignored
make lint
  PASS: rustfmt and workspace/all-target Clippy with warnings denied
git diff --check
  PASS
```

The ordinary `make test` and `make spec` remain the coordinator's final gate
after independent adversarial code review.

## Adversarial Code Review

Reviewer: `/root/model_architecture_research`

Initial verdict: `REJECT`.

The reviewer found three Major proof gaps:

1. The first dependency check read workspace-global `cargo metadata`, so an
   unrelated consumer could unify extra features into both projections. The
   SNV root declaration also omitted its direct `libc` use.
2. The source inventories did not close over `CommandError` in
   `production.rs` or the asset error/input-audit definitions used by
   `assets/snv.rs`.
3. The claimed recomputation called the production fingerprint function and
   therefore was not an independent digest oracle.

Developer disposition:

1. Replaced workspace resolution with standalone per-artifact temporary Cargo
   projects generated from exact roots/default-features/features and checked
   isolated lock snapshots. `cargo metadata --locked --offline` now resolves
   only that artifact. Source-use evidence and mutation controls require
   `libc` and every other direct root.
2. Extracted shared `CommandError`, asset errors, and the input-open audit into
   bounded modules and included them in every causal inventory that imports
   them.
3. Added hard expected SNV/reference digests and a separate one-shot oracle
   that constructs the complete framed preimage through a `BTreeMap` without
   calling the production incremental hasher.

Re-review: pending with the same reviewer.

The first re-review remained `REJECT`: although isolated Cargo resolution and
the independent digest oracle were fixed, direct-root completeness still
compared two checked declarations and never bound version/default-feature/
feature settings to the actual relevant Cargo manifests. The reviewer also
identified the narrow shared `CommandError` extraction as outside the v1
contract's three-extraction limit. The replacement remediation contract above
explicitly admits only the bounded support extractions already required for
source closure and requires a source- and manifest-derived dependency control.
The original design reviewer must accept the exact replacement identity before
the developer resumes.

## External Effect Evidence

Coordinator: not applicable. The ticket changes local source, tests, miniature
fixtures, and documentation only. It performs no network operation, release,
deployment, publication, production build, or production-data read.

## Coordinator Final Check

Coordinator: pending

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Artifact-local fingerprints | `cargo test --locked -p pangopup-build source_fingerprint -- --nocapture` | pass: 7 unit + 4 integration |
| Legacy v1 readability | `builder_provenance` real legacy-manifest controls | pass: SNV lookup + reference window |
| SNV payload stability | checked ten-file migration evidence + 1,000-case regression | pass |
| Reference payload stability | exact miniature baseline member/notice comparison | pass |
| Mask identity preserved | exact compiled fingerprint assertion + mask miniature controls | pass: `fd738fec...6816` |
| Production identities untouched | source-only release/reference constant checks; no production open | pass |
| Documentation/current frontier | ADR 0012, evidence, README/architecture/frontier/issue update | implementation complete; review pending |
| Repository gate | `make lint`; `make test`; `make spec` | lint pass; test/spec pending final coordinator gate |
