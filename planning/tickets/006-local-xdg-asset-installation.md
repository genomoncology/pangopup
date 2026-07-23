# 006 — Install and reuse the SNV index from Linux XDG storage

Status: complete

## Why

Ticket 005 shipped a deterministic, release-sized transport for the already
built and certified 15,033,158,255-byte SNV index. The next useful slice is to
install those local transport files once, remember the active bundle, and let
ordinary lookup use it without a 15 GB rescan or an explicit `--bundle` on
every command.

The former Ticket 006 draft made exhaustive validation and a public `--full`
verification path part of routine installation. That conflicts with the
current product decision. Transfer hashes protect the bytes; cheap structural
open protects runtime safety; a bounded source-derived regression corpus proves
that lookup returns the expected answers. No routine whole-index semantic scan
is introduced by this ticket.

## Scope

- Extend `pangopup-assets` with a Linux local asset store and keep filesystem,
  transport, receipt, locking, and activation logic out of `pangopup-core` and
  `pangopup-index`.
- Resolve the complete data root in this order:
  1. explicit `--data-dir <ABSOLUTE_PATH>`;
  2. nonempty absolute `PANGOPUP_DATA_DIR`;
  3. nonempty absolute `XDG_DATA_HOME` plus `/pangopup`;
  4. nonempty absolute `HOME` plus `/.local/share/pangopup`;
  5. otherwise fail `PATH_UNAVAILABLE`.
  Empty, relative, or non-UTF-8 explicit/environment paths fail
  `PATH_INVALID`; they never fall through or become relative to the current
  directory. Tests isolate every environment variable and never use the real
  user data directory.
- The data root is a private local-user trust boundary. Create it mode `0700`;
  if it exists, require a real directory owned by the effective uid with no
  group/world write bits. Open it once as a directory handle and perform every
  operation below it relative to that handle with no-follow semantics. Reject
  symlinked/non-directory lock, state, staging, bundle, receipt, and member
  paths. Data-root metadata and locks are mode `0600`; staging directories are
  `0700`; installed members and receipts are `0444`; published bundle
  directories are `0555`. Never traverse mount points or links during cleanup.
- Use this installed layout, where `<B>` is the 64 lowercase hexadecimal
  portion of the bundle ID:

  ```text
  <data>/bundles/<B>/bundle/{NOTICE,manifest.json,scores.pgi}
  <data>/bundles/<B>/receipt.json
  <data>/active.json
  <data>/.install.lock
  <data>/.staging/<random>/...
  ```

  `receipt.json` binds schema `pangopup.install-receipt.v1`, bundle ID,
  transport ID, and the three installed member descriptors. `active.json`
  binds schema `pangopup.active-profile.v1` and bundle ID. Both are strict,
  RFC 8785 canonical JSON with no timestamps or host-specific fields. Every
  identity is exactly `sha256:` plus 64 lowercase hexadecimal characters;
  every integer is in `0..=2^53-1`. Their complete logical objects are:

  ```text
  receipt.json = {
    schema: "pangopup.install-receipt.v1",
    bundle_id: <sha256>,
    transport_id: <sha256>,
    members: [
      {path:"bundle/NOTICE", size:<u53>, sha256:<sha256>},
      {path:"bundle/manifest.json", size:<u53>, sha256:<sha256>},
      {path:"bundle/scores.pgi", size:<u53>, sha256:<sha256>}
    ]
  }
  active.json = {
    schema: "pangopup.active-profile.v1",
    bundle_id: <sha256>
  }
  ```

  Unknown, missing, duplicate, reordered member, noncanonical, or trailing
  content is invalid. Array member order is exactly the order above; RFC 8785
  owns serialized object-member order.
- Each stage is `.staging/<32-lowercase-hex-nonce>/` with a mode-`0400`
  `marker.json` binding schema `pangopup.install-stage.v1`, nonce, effective
  uid, bundle ID, and transport ID, plus `payload/` containing the prospective
  `<B>` directory and `active.candidate.json` containing the prospective active
  profile. Its strict RFC 8785 object is exactly
  `{schema:"pangopup.install-stage.v1",nonce:<32-lower-hex>,euid:<u53>,bundle_id:<sha256>,transport_id:<sha256>}`
  with the same identity/integer rules as the receipt. While holding the install lock, reconciliation may remove a
  direct stage child only when it is a same-filesystem, effective-uid-owned real
  directory whose strict no-follow marker matches its directory name and
  intended identities. A malformed/unowned stage returns `STAGING_INVALID` and
  is not touched.
- Add these exact public commands to the `pangopup` executable:

  ```text
  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
  pangopup assets status [--data-dir <ABSOLUTE_PATH>]
  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>]
    --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...]
    [--gene <ENSG>] [--format jsonl|table]
  ```

  An explicit `--bundle` preserves the shipped development/offline override and
  is mutually exclusive with `--data-dir`. Without `--bundle`, lookup resolves
  the active installed bundle. The existing lookup result bytes remain
  unchanged.
- `assets install` obtains one data-root lock, validates the closed Ticket 005
  transport manifest and member set, streams each compressed byte once while
  checking declared hashes and reconstructing the exact bundle in private
  staging, accumulates the installed `scores.pgi` SHA-256 during that same
  decompression/write stream, hashes `NOTICE` and `manifest.json` from the
  already bounded in-memory bytes used to write them, and runs only
  `BundleOpen::open` structural/compatibility validation afterward. It must not
  call `certify_bundle`, `visit_all`, `pangopup-build verify`, or otherwise
  traverse or rehash the decompressed payload a second time. Test-only counted readers
  prove each compressed byte is consumed once, no score-member read occurs
  between completed write and cheap open, and reuse opens no transport part.
- Publish the staged `<B>` directory with a same-filesystem no-replace rename,
  but first apply final modes to all installed files and the inner `bundle/`
  directory, `sync_all` each of `scores.pgi`, `NOTICE`, `manifest.json`, and
  `receipt.json`, and fsync the staged `bundle/` and `<B>/` directories. Keep
  the enclosing `<B>` mode `0700` for the cross-parent rename because Linux
  rejects moving a mode-`0555` directory. Immediately after rename, use the
  already-held `<B>` directory handle to change it to final mode `0555`, fsync
  `<B>`, and fsync the bundles directory before touching the active profile. A
  crash in this narrow post-rename/pre-chmod window leaves the valid marked
  stage and a complete inactive mode-`0700` final; reconciliation may finish
  only that marker-bound chmod/fsync operation. A mode-`0700` final without the
  exact owned stage marker is `INSTALL_CONFLICT` and is never repaired. Then sync
  `active.candidate.json` inside the marked nonce stage, atomically rename that
  file over root `active.json`, and fsync the root directory. A crash before
  bundle rename leaves only a removable marked stage. A crash after bundle
  rename but before active rename leaves a complete inactive immutable bundle
  plus an owned candidate; the next install validates the final bundle and
  activates the candidate only when the new request has the same bundle and
  transport identities. A different new request deletes the validated stale
  candidate, retains the published old bundle as inactive, cleans the marked
  stage, and proceeds. A crash after active rename leaves a valid active bundle; the next install
  cleans the remaining marker/wrapper and returns `reused`. No recovery path
  deletes or overwrites a published bundle.
- After the active rename and root fsync form the durable commit point, normal
  cleanup removes the now-empty `payload/`, unlinks the matching marker, removes
  the nonce wrapper, and fsyncs `.staging`. Cleanup after that commit point is
  best-effort: failure does not turn a durably installed/active result into an
  error, and the next install retries reconciliation. Reconciliation examines
  every direct staging child by its own marker identities and may remove any
  valid stale Pangopup stage, including one for a different bundle/transport
  than the current request; it never requires the marker to match the current
  install. Order cleanup as: remove empty `payload/`, unlink the marker, remove
  the wrapper, fsync `.staging`. The only markerless reconciliation exception is
  a direct nonce-shaped, same-filesystem, effective-uid-owned real wrapper whose
  no-follow enumeration proves it completely empty; remove that wrapper with
  `rmdir`. A markerless wrapper containing anything is `STAGING_INVALID` and is
  untouched. Before the commit point, cleanup failure is `ASSET_IO` and nothing
  becomes active.
- Installation takes `flock(LOCK_EX|LOCK_NB)` on the no-follow regular
  `.install.lock` and immediately fails `ASSET_LOCKED` when another installer
  owns it; there is no timeout or waiting loop. Lookup is lock-free and keeps
  using the last atomically published active bundle during an install. Status
  probes the lock nonblockingly only to report `installing`; it never waits or
  reads staging. Reinstalling the same bundle cheaply validates its receipt,
  manifest, sizes, regular-file shape, and `BundleOpen`, then returns `reused`
  without opening transport parts or hashing `scores.pgi`. Installing another
  valid bundle publishes it immutably and moves the active profile atomically;
  old bundles remain available. Garbage collection and rollback commands are
  later work.
- Successful stdout is one compact LF-terminated JSON object with stable key
  order and empty stderr:
  - install: `status` (`installed` or `reused`), `bundle_id`, `transport_id`,
    `path`;
  - ready status: `status=ready`, `bundle_id`, `transport_id`, `path`,
    `installing` (`true` or `false`);
  - installing without an active bundle: `status=installing`, `data_dir`;
  - missing status: `status=missing`, `data_dir` and exit 0.
  Every successful install/ready `path` is the absolute three-member bundle
  path `<data>/bundles/<B>/bundle`, never the enclosing `<B>` receipt directory.
  Failures emit the existing compact error object on stderr and no stdout.
  Preserve Ticket 005's transport error codes. Add `PATH_INVALID` and
  `PATH_UNAVAILABLE` as usage/configuration errors with exit 2, plus
  `ASSET_LOCKED`, `ASSET_IO`, `ASSET_STATE_INVALID`, `STAGING_INVALID`, and
  `INSTALL_CONFLICT` as operational errors with exit 1. Lookup without an
  active bundle fails `ASSETS_MISSING` with exit 1. The closed command matrix is:

  | Condition | `assets install` | `assets status` | implicit lookup |
  |---|---|---|---|
  | no root or no `active.json` | proceed/create | missing, exit 0 | `ASSETS_MISSING`, exit 1 |
  | another installer, no active | `ASSET_LOCKED`, exit 1 | installing, exit 0 | `ASSETS_MISSING`, exit 1 |
  | another installer, valid active | `ASSET_LOCKED`, exit 1 | ready + `installing=true`, exit 0 | use active |
  | valid active, unlocked | install/reuse | ready + `installing=false`, exit 0 | use active |
  | malformed/nonregular active, missing selected bundle, receipt/member mismatch | `ASSET_STATE_INVALID`, exit 1 before replacement | `ASSET_STATE_INVALID`, exit 1 | `ASSET_STATE_INVALID`, exit 1 |
  | structurally incompatible selected bundle | `INSTALL_CONFLICT`, exit 1 | existing `BUNDLE_INCOMPATIBLE`, exit 1 | existing `BUNDLE_INCOMPATIBLE`, exit 1 |
  | other data-root I/O | `ASSET_IO`, exit 1 | `ASSET_IO`, exit 1 | `ASSET_IO`, exit 1 |
  | malformed/unowned staging | `STAGING_INVALID`, exit 1 | ignored | ignored |

  Status reports the current immutable active state and the lock probe; it does
  not validate or expose the in-progress candidate. `--bundle` and `--data-dir`
  are mutually exclusive; supplying both is `CLI_USAGE` exit 2. There is no
  precedence between them.
- Add a checked-in source-derived regression fixture containing exactly 1,000
  deterministic SNV requests and their expected gene-specific score records.
  Its source is the six existing attributed Zenodo excerpts under
  `tests/fixtures/pangolin-precompute/`, sorted by filename with rows kept in
  file order. Select 970 filtered hit requests by round-robin across the six
  files, skipping `REF=N` rows, skipping the six fixed filtered requests below,
  and skipping a file after exhaustion; each request uses that row's exact
  gene, contig, position, REF, ALT, score, and positions. Append 30 fixed edge
  requests in this order: 12 unfiltered requests for the four lowest genomic
  WRAP53/TP53 overlapping loci (`chr17:7686072..7686075`) and their three
  published ALTs; six alternating
  WRAP53/TP53 filtered requests (all three ALTs at chr17:7686072 filtered to
  ENSG00000141499, then all three at chr17:7686073 filtered to
  ENSG00000141510); six unfiltered REF=N ambiguity requests
  (`chr10:114306066 A>{C,G,T}` and `chr12:122093260 T>{A,C,G}`); and six
  deterministic misses (`chr10:1 A>C`, `chr10:1 A>G`, `chr12:1 A>C`,
  `chr12:1 A>G`, `chr13:1 A>C`, and `chr17:1 A>C`). If an intended
  edge candidate is not valid in the pinned excerpts, the coordinator and
  ticket reviewer—not the developer alone—must amend this selection contract.
- Generate expected JSONL by a direct strict TSV parser/join that implements the
  documented centi-score and overlap rules without calling `BundleOpen`,
  `ScoreProvider`, `render_requests`, or reading the generated bundle. Generate
  the small fixed-v1 bundle independently from the closure of source loci
  needed by those requests: include all three published ALT rows and every
  source gene in the six excerpts at each selected genomic locus, so an
  unfiltered query cannot be made falsely single-gene by fixture selection.
  Commit both under `tests/fixtures/snv-regression/` with provenance and a
  regeneration test/tool that reproduces their byte identities exactly. This
  prevents the code under test from blessing its own output. Normal tests must
  not require the downloaded raw corpus or retained 15 GB bundle.
- Open the small bundle once and run all 1,000 expectations through the real
  `ScoreProvider` path in one Rust integration test. The current CLI has one
  batch-wide `--gene`, so exercise the same corpus through seven CLI batches:
  one batch for all unfiltered requests and one batch for each of the six gene
  filters. Compare each group's complete JSONL against the direct-TSV oracle
  subset in original per-group order; do not add a new per-request input format
  in this ticket. Retain
  a non-gating timing report in
  `planning/artifacts/006-local-install-and-regression.md`. Add a fixed-fixture
  `snv_regression` bench invoked exactly as
  `cargo bench --locked --package pangopup-cli --bench snv_regression`; it
  reports fresh open/process behavior plus warm 1, 10, 100, and 1,000 request
  batches. Do not call page-cache state cold unless the harness controls
  residency. The report includes lookup/result counts, p50/p95/p99, allocation
  calls/bytes, page faults, RSS caveat, and output bytes. Timing is non-gating;
  the ordinary semantic regression target is about two seconds on the current
  development machine, but correctness CI must not use a hardware-specific
  wall-clock assertion.
- Add a minimal GitHub Actions workflow that installs the repository's pinned
  Rust toolchain from `rust-toolchain.toml`, pins the setup-uv action by full
  commit with uv `0.8.0`, installs `mustmatch==0.1.0` using `uv tool install`,
  then runs only `make lint`, `make test`, and `make spec` on Linux. It must use
  checked-in small fixtures, have no production-asset credentials, and never
  download the production dataset or production index.
- Add the executable outside-in contract at `spec/local-assets.md` and the CI
  workflow at `.github/workflows/ci.yml`.
- Update `README.md`, `architecture/runtime-data.md`, `architecture/delivery.md`,
  `architecture/design.md`, `architecture/index.md`, `architecture/source-data.md`,
  `architecture/README.md`, ADR 0004, ADR 0005, `planning/faq.md`, and
  `planning/frontier.md` to mark local Linux installation, active-bundle
  discovery, cheap reuse, and the fast regression corpus as shipped. Keep
  remote sync/publication, download progress, HTTP, containers, models,
  inference, and model-result caching explicitly future.
- Excluded: network access, GitHub release publication, download cache/resume,
  signatures, general repair/GC/rollback, macOS/Windows support claims,
  reference/model/mask assets, model inference, HTTP, Docker, and any new
  whole-index verifier.

## Success Checklist

- A local Ticket 005 transport installs atomically into an isolated Linux XDG
  root, becomes active, and is reused without transport files or a payload
  scan.
- Lookup without `--bundle` returns byte-identical existing JSONL/table output
  from the active bundle; the explicit bundle override remains compatible.
- Status distinguishes missing, ready, corrupt/conflicting, and locked states
  with stable machine-readable behavior.
- Unit/integration/spec coverage proves path precedence, invalid paths,
  transport corruption, interrupted publication, concurrency, idempotent reuse,
  active-profile atomicity, symlink/non-regular rejection, absence of a second
  payload pass, and transactional stdout.
- Exactly 1,000 checked-in source-derived expectations pass through the library
  and batched CLI using only a small checked-in bundle. The retained benchmark
  reports open and 1/10/100/1,000 request behavior without becoming a flaky CI
  threshold.
- GitHub CI and local `make lint`, `make test`, and `make spec` pass without the
  production dataset or retained production asset.

## Decisions

1. **Prove answers with a bounded corpus, not a whole-index rerun.** A complete
   payload traversal is expensive and duplicates build-time certification.
   Options were exhaustive install validation, sampled runtime validation, or
   transfer integrity plus a checked-in semantic corpus. Use the third: hashes
   establish exact bytes, cheap open establishes format safety, and 1,000
   representative cases establish observable lookup behavior.
2. **Install and activate one default profile now.** Requiring `--bundle` is
   explicit but unsuitable for automatic service startup; scanning installed
   directories makes selection ambiguous. Use a strict atomically replaced
   `active.json`, while retaining `--bundle` as the explicit override.
3. **Keep installed data in XDG data, not cache.** The 15 GB mmap file is
   authoritative durable data; cache cleanup must not remove it. Only later
   partial network downloads belong in XDG cache.
4. **Serialize installation with one root lock.** Per-bundle plus activation
   locks allow more concurrency but complicate crash ordering for an operation
   performed rarely. One lock makes bundle publication and active-profile
   change a single understandable transaction without affecting lookup.
5. **Stream installation once.** Calling existing exhaustive unpack is easy but
   rereads the 15 GB result. Refactor shared transport decoding so the local
   installer checks every transported byte while writing once and then performs
   cheap structural open only. Builder certification remains a build-time tool,
   not a startup/install habit.
6. **Use real source excerpts but a tiny runtime fixture.** Tests against the
   production mmap would be realistic but non-portable and tempting to rescan.
   A deterministic 1,000-query fixed-v1 fixture preserves real published score
   values while keeping tests self-contained and fast.
7. **Linux first.** Linux/XDG is the current service and container target.
   Pretending atomic/durability behavior is portable would weaken the contract;
   macOS/Windows behavior is a later measured slice.
8. **CI verifies code, not production delivery.** The normal gate uses only
   checked-in fixtures. Clean-machine production asset publication and download
   proof belongs to the next remote-sync ticket.

## Dependencies

Ticket 005, shipped by commits `4161679` and `80eaaba`.

## Notes

- The retained production bundle and transport under the workspace data area
  are inputs for later publication. Do not rebuild, move, rewrite, hash-scan, or
  delete them for this ticket.
- `pangopup-build verify` remains a builder/certification tool, but this ticket
  does not invoke it from install, status, lookup, tests, or CI and does not add
  a public `assets verify --full` command.
- The source excerpts already carry the Zenodo DOI, archive identity, selection
  description, and CC BY 4.0 attribution. Preserve those notices in the new
  regression fixture.
- Existing specs are canonical for lookup JSON, contig aliases, batch ordering,
  exit behavior, and transactionality. Do not silently change them.
- The exact gate is `make lint`, `make test`, and `make spec`; there is no
  `make check`. Public files must contain no absolute developer paths or
  GenomOncology-internal software references. Evidence in this ticket is
  instructional and is not itself a benchmark result.
- If an appropriate shared transport decoder or fixture writer already exists,
  reuse it; otherwise define it within the owning crate rather than copying
  codec logic into the CLI.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-07-23

This ticket replaces the pre-Ticket-005 draft and incorporates Ian's explicit
decisions: preserve the built asset, remove routine exhaustive verification,
use a fast representative JSONL suite, and create tickets one at a time from
the previous result and full roadmap.

## Independent Ticket Review

Reviewer: `/root/ticket_006_design_review`

First review: NOT APPROVED.

1. The Linux trust/crash contract was underspecified. Resolved by defining the
   private effective-uid-owned root, dirfd-relative no-follow operations,
   modes, strict stage marker, safe cleanup boundary, exact publish/fsync
   ordering, and before/after-rename recovery states.
2. Status and errors were incomplete. Resolved with a closed command/condition
   matrix, exact lock probing, JSON states, added error codes, and explicit
   mutual exclusion of `--bundle`/`--data-dir`.
3. The oracle risked circular generation. Resolved with an exact 970+30
   selection, direct-TSV expected-output path forbidden from using runtime
   lookup/rendering, and byte-reproducible independent bundle generation.
4. The one-pass claim lacked proof. Resolved by accumulating the score hash in
   the decompression/write pass and requiring counted-I/O tests for install and
   reuse.
5. Lock behavior was unresolved. Resolved as immediate nonblocking flock,
   lock-free lookup, and nonblocking status reporting.
6. Performance evidence was unnamed. Resolved with the exact retained artifact,
   benchmark target/command, workloads, metrics, and page-cache wording.
7. CI dependencies were not pinned. Resolved with Linux, the pinned repository
   toolchain, commit-pinned setup-uv configured for uv 0.8.0, and
   `mustmatch==0.1.0`.
8. Scope was large. Retained as one coherent vertical slice after bounding the
   filesystem state machine and independent regression fixture; CI is only the
   existing three gates and all production/network work remains excluded.

Second review: NOT APPROVED.

1. One mixed CLI batch was impossible because `--gene` is batch-wide. Resolved
   by requiring one unfiltered and six per-gene batches without expanding the
   shipped input grammar.
2. Successful stage cleanup was incomplete. Resolved with an exact post-commit
   unlink/rmdir/fsync sequence, success after the durable commit point, and
   identity-based reconciliation of stale stages from any prior request.
3. Several durable documents were unnamed. Resolved by naming every known stale
   architecture page plus `spec/local-assets.md` and
   `.github/workflows/ci.yml`.
4. Receipt/profile schemas were incomplete. Resolved with complete logical
   objects, exact identities, integer bounds, paths, member order, canonical
   serialization, and strict rejection rules; the stage marker is equally
   pinned.

Third review: NOT APPROVED.

1. Pre-publication data durability was incomplete. Resolved by applying final
   modes, syncing all four files, and fsyncing both staged directories before
   the bundle rename.
2. The active candidate could be stranded at the root. Resolved by placing it
   in the marked nonce stage and defining same-request activation versus
   different-request discard during reconciliation.
3. `path` was ambiguous. Resolved as the absolute
   `<data>/bundles/<B>/bundle` directory.
4. Two stale architecture pages were missing. Resolved by adding
   `architecture/source-data.md` and ADR 0004 to the implementation list.

Approval re-review: APPROVED. The reviewer reopened the current file after the
marker-last cleanup correction and reported no residual material findings. The
ticket is decision-complete, internally consistent, and ready for development.

Implementation amendment: while implementing on Linux, the developer proved
that cross-parent rename of the enclosing staged `<B>` fails with `EACCES` once
that directory is mode `0555`. The coordinator changed only the reviewed mode
ordering: files and inner directory are final and durable first; `<B>` remains
`0700` for rename, becomes `0555` through its held handle immediately after,
and is durable before activation. Exact marker-bound recovery for a crash in
that window is now specified. Same-reviewer approval: APPROVED. The reviewer
confirmed the writable directory exists only while private/inactive, the held-
handle chmod and fsync precede activation, marker-bound recovery is safe, and
unmarked mode-`0700` finals fail closed.

## Implementation Evidence

Developer: Codex `/root/ticket_006_developer`, 2026-07-23

Implementation outcome:

- Added the Linux local-user store to `pangopup-assets`: strict path
  precedence/validation, effective-uid and mode checks, one opened root
  directory handle, component-relative no-follow operations, nonblocking
  `flock`, canonical receipt/profile/stage schemas, marked reconciliation,
  one-stream transport reconstruction, immutable no-replace publication,
  held-handle final-mode durability, atomic activation, cheap active/reuse
  validation, and best-effort post-commit cleanup. Install/status/lookup never
  invoke `certify_bundle`, `visit_all`, or another complete score pass.
- Added `pangopup assets install`, `pangopup assets status`, implicit active
  lookup, the explicit bundle override, the complete requested JSON/error
  boundary, and transactional stdout. Manual and executable tests confirmed
  reuse while the transport payload part was mode `000`.
- Added unit/integration coverage for path precedence and failure, canonical
  closed JSON, private state shape/modes, one-pass counted compressed reads,
  direct transition from completed score write to cheap open, part-free reuse,
  locking/status, valid and markerless reconciliation, malformed-stage
  preservation, unsafe root state, symlink rejection, install/status/lookup,
  corruption, and mutual exclusion. The focused executable contract at
  `spec/local-assets.md` passed 12/12 blocks.
- Added the exact source-derived 970+30 fixture at
  `tests/fixtures/snv-regression/` (bundle
  `sha256:5d90c69bb9220fbbbc2dd30fcf6cd0c1f88037d891fb79fe7e01df5d9f08c624`).
  Its direct strict-TSV generator does not call the reader, provider, or
  renderer. A reproduction test rebuilds every fixture file and compares byte
  identities. One real provider open passed all 1,000 expectations, and all
  seven CLI batches matched their complete oracle subsets; the focused run
  completed in 0.04 seconds. A third CLI regression proves non-UTF-8 lookup
  `--data-dir` values reach `PATH_INVALID`, not `CLI_USAGE`.
- Added the exact `snv_regression` benchmark target and retained
  `planning/artifacts/006-local-install-and-regression.md`. The required
  benchmark command passed and reported fresh open/process plus warm
  1/10/100/1,000 batches, p50/p95/p99, results, allocation calls/bytes, page
  faults, RSS caveat, and output bytes without a timing gate or cold claim.
- Added `.github/workflows/ci.yml`: Linux, exact Rust `1.93.1` minimal profile
  with the pinned `clippy` and `rustfmt` components from `rust-toolchain.toml`,
  full-commit-pinned `astral-sh/setup-uv`, uv `0.8.0`,
  `mustmatch==0.1.0`, and only the three repository gates. It uses no
  production credentials or production assets.

Documentation changed:

- User/executable: `README.md`, `spec/cli.md`, and `spec/local-assets.md`.
- Durable architecture: `architecture/runtime-data.md`,
  `architecture/delivery.md`, `architecture/design.md`,
  `architecture/index.md`, `architecture/source-data.md`,
  `architecture/README.md`, ADR 0004, and ADR 0005.
- Planning/evidence: `planning/faq.md`, `planning/frontier.md`, and
  `planning/artifacts/006-local-install-and-regression.md`.

Verification evidence on the final review diff:

- `cargo test --locked -p pangopup-assets --lib -- --test-threads=1` — 19
  passed, including the counted one-pass audit, every named durability
  fault/crash window, candidate recovery/discard, exact wrapper modes,
  two-phase reconciliation, post-commit cleanup success, and local-write
  `ASSET_IO` classification.
- `cargo test --locked -p pangopup-index --lib
  held_member_open_survives_bundle_path_rename` — passed; handle-based open
  succeeded after the original bundle path ceased to exist.
- `cargo test --locked -p pangopup-build --test local_install` — 2 passed.
- `cargo test --locked -p pangopup-build --test snv_regression_fixture` — 1
  byte-exact regeneration test passed.
- `cargo test --locked -p pangopup-cli --test snv_regression` — 3 passed
  (1,000 provider expectations, seven CLI batches, and non-UTF-8 data path).
- `cargo bench --locked --package pangopup-cli --bench snv_regression` —
  passed; retained non-gating results are in the named artifact.
- `make lint` — passed (`cargo fmt --check`; clippy all targets with warnings
  denied).
- `make test` — passed across the workspace.
- `make spec` — 92 passed.
- `git diff --check` — passed. Public implementation/docs/fixtures contain no
  absolute developer path or internal-project reference. The retained
  production bundle/transport was not touched, rebuilt, moved, hashed, or
  scanned.

## Adversarial Code Review

Reviewer: Codex `/root/ticket_006_code_review`, 2026-07-23

Initial result: NOT APPROVED. All nine findings were treated as material and
resolved on the same intended diff:

1. **Mutable wrapper reuse.** Ordinary reuse and active discovery now require
   the enclosing `<B>` wrapper to be exactly `0555`; an unmarked `0700`
   wrapper returns `INSTALL_CONFLICT`. Only a fully preflighted marker-bound
   crash stage may accept `0700`, and recovery uses its held descriptor to
   chmod `0555`, fsync the wrapper, and fsync `bundles/` before activation.
2. **Incomplete pre-publication durability.** Each member receives its final
   mode before its file fsync. The marker file, nonce directory, `.staging`,
   active candidate, candidate directory, inner bundle, wrapper, and payload
   receive the required fsyncs before the bundle rename.
3. **Reconciliation mutated before validation and could fail after commit.**
   Reconciliation is now a read-only preflight of every staging child followed
   by mutation. Recovered active rename plus root fsync is its sole commit;
   every cleanup afterward is best effort, and committed recovery returns
   `reused` even when cleanup is injected to fail.
4. **Ambient absolute reopens and mount traversal.** Every path below the held
   root uses `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS |
   RESOLVE_NO_MAGICLINKS | RESOLVE_NO_XDEV`, so bind mounts are rejected even
   when their `st_dev` matches. Directory enumeration uses held-descriptor
   `getdents`, not `/proc/self/fd`. `BundleOpen::open_members` and
   `IndexReader::open_file` consume verified held files, including implicit
   CLI lookup; a rename test proves the old absolute path is not reopened.
5. **CI did not install the pin.** CI now explicitly installs Rust `1.93.1`,
   profile `minimal`, and components `clippy,rustfmt`, exactly matching
   `rust-toolchain.toml`, before the three gates.
6. **Wrong error boundaries.** Injected local score-destination failure is
   remapped from generic decoder `OUTPUT_IO` to `ASSET_IO`; lookup preserves a
   non-UTF-8 `--data-dir` until strict path resolution returns `PATH_INVALID`.
7. **Stale CLI benchmark and percentile ambiguity.** Every benchmark run now
   rebuilds the current CLI artifact. Percentiles use and document the
   nearest-rank `ceil(p*n)-1` rule; the retained report was regenerated.
8. **Missing deterministic crash coverage.** The test-only fault matrix covers
   every marker/candidate/member/wrapper chmod and fsync boundary, both
   renames, root fsync, pre-publication discard, marker-bound recovery,
   unmarked `0700`, read-only all-child preflight, counted one-pass score
   construction, and successful commit despite cleanup failure.
9. **Stale/malformed documentation.** `AGENTS.md` now lists shipped Linux local
   installation/discovery, and the malformed distribution paragraph in
   `architecture/source-data.md` is corrected.

First remediation re-review: NOT APPROVED with two residual findings. Both are
resolved without broadening the ticket:

1. **Recovery erased installed-state error boundaries.** Reconciliation now
   propagates `validate_installed` unchanged. Receipt/member mismatches remain
   `ASSET_STATE_INVALID`; only marker/stage defects and marker-to-receipt
   identity conflicts are `STAGING_INVALID`; fixed-index structural failures,
   including corrupted scores magic, are `INSTALL_CONFLICT`. A marker-bound
   recovery test asserts both installed-state and conflict codes.
2. **The score-read proof inferred absence from adjacent events.** The index
   now exposes a feature-gated test-only logical-read counter at the actual
   header, segment, tree, and exception decode points. Installation audit
   snapshots assert exactly zero score bytes at completed write and again at
   `CheapOpenStart`, then a positive counted total only after cheap structural
   open completes. The former event-adjacency assertion was removed.

Same-reviewer second re-review: APPROVED with no residual findings. The
reviewer confirmed both residual fixes, reran their focused cases, and verified
that every previously approved durability, confinement, fixture, CI, benchmark,
documentation, and hygiene remediation remains intact. No production asset was
touched or scanned.

## Coordinator Final Check

Coordinator: Codex `/root`, 2026-07-23

- Independent ticket review and its implementation-order amendment are
  approved.
- Independent adversarial code review approved the final remediation diff with
  no residual findings.
- Coordinator `make lint` passed.
- Coordinator `make test` passed across the workspace, including 19 asset
  unit/fault tests, byte-exact regeneration, and all 1,000 lookup cases.
- Coordinator `make spec` passed: 92/92.
- `git diff --check` passed. The stale-claim/public-hygiene scan found no local
  absolute paths or internal-project references outside this ticket's own
  explicit hygiene instruction.
- The checked-in regression oracle is exactly 1,000 lines; its largest member
  is 491,629 bytes. No production bundle or transport was touched, scanned, or
  added.
