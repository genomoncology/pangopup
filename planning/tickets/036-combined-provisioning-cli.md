# 036 — Synchronize and report the complete installed splice runtime

Status: complete

## Why

Pangopup now has two independently hardened provisioning operations: one for
the immutable SNV lookup and one for the compatible model, reference, and mask
runtime. Users still have to know the historical nested SNV-only commands, and
there is no single answer to “is Pangopup ready?” This ticket composes the two
established operations behind the ordinary top-level CLI without changing
either transport or rebuilding any asset.

This is the next coherent slice because the runtime sync primitive is shipped,
while an installer or HTTP service should be able to direct users to one stable
`pangopup sync` and `pangopup status` contract.

## Scope

- Add `pangopup sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir
  <ABSOLUTE_PATH>]`. Resolve all path inputs before acquiring or attempting
  work. Synchronize the pinned SNV release first, then the pinned runtime
  release, because runtime activation requires the compatible active SNV.
- Add typed `pangopup-assets` composition boundaries for whole-product sync and
  status so the CLI and future HTTP adapter do not independently invent state
  policy. Preserve the existing typed SNV/runtime operations as the owning
  delivery primitives. Add one bounded, offline-only runtime-cache inspection
  primitive used only when the required SNV is missing; it takes the existing
  runtime cache lock, performs the same closed member/sidecar inventory checks
  as sync, and returns `complete` or the existing bounded missing/corrupt error.
  It never installs, hashes/decodes a large member, or contacts the network.
- Successful sync writes one compact JSON line with keys in this order:
  `status="ready"`, nested `snv`, nested `runtime`, aggregate
  `downloaded_bytes`, and aggregate `resumed_bytes`. The nested objects retain
  the existing complete typed outcomes and their `installed`/`reused` status;
  aggregate counters use checked addition.
- A failed component sync writes no stdout, exits 1, and writes one compact
  stderr object with `status="error"`, code `ASSET_SYNC_INCOMPLETE`, message
  `asset synchronization did not complete`, and `details` containing `snv`
  and `runtime` in that order. A component entry is either its complete success
  outcome, `{status:"error",code,message}`, or
  `{status:"not_attempted",reason:"snv_sync_failed"}`.
  - Online mode does not attempt runtime sync after any SNV failure.
  - Offline mode uses the bounded runtime-cache inspection after an SNV
    `ASSETS_MISSING` result. An incomplete/corrupt runtime cache contributes its
    existing error. A closed runtime cache contributes exactly
    `{status:"not_attempted",reason:"snv_sync_failed",cache:"complete"}`;
    it is not called installed or ready because activation was impossible.
  - Other SNV failures do not attempt the dependent runtime operation.
  - Completed installation remains selected; there is no rollback of valid
    gigabyte-scale work, and retry converges through existing reuse/resume.
- Serialize top-level sync calls with one Linux, data-root-scoped,
  nonblocking provisioning lock held across both component operations. The
  lock is separate from cache/install locks and never blocks lookup. A
  concurrent loser attempts neither component and returns the ordinary exact
  error line
  `{status:"error",code:"ASSET_LOCKED",message:"another Pangopup synchronization is in progress",details:null}`.
  Lock order is provisioning
  lock, SNV cache/install as needed, then runtime cache/install as needed.
  Descriptor release after return or crash ends the operation; add no durable
  progress database or marker.
- Add `pangopup status [--data-dir <ABSOLUTE_PATH>]`. It performs no network
  work and emits one compact object with ordered keys `status`, `data_dir`,
  `syncing`, `installing`, `snv`, and `runtime`.
  - Component objects are exactly:
    - SNV ready: `{status:"ready",bundle_id,transport_id,path}`;
    - runtime ready:
      `{status:"ready",profile_id,snv_bundle_id,model_bundle_id,reference_bundle_id,mask_sha256,model_path,reference_path,mask_path}`;
    - either installing or missing: `{status:"installing"}` or
      `{status:"missing"}`;
    - error observation:
      `{status:"error",code,message}`.
    Do not expose a runtime profile-directory path, duplicate `data_dir`, or
    duplicate operation flags inside component objects.
  - Aggregate `status` describes usable installed state: `ready` only when
    both compatible components are ready, `partial` when exactly one is ready,
    and `missing` when neither is ready. `syncing` probes the provisioning lock;
    `installing` reflects the existing shared install lock. Missing, partial,
    and in-progress observations exit 0.
  - Obtain a coherent idle snapshot by taking the existing shared install lock
    nonblockingly and holding it while both active states are read. If that lock
    is already held, set `installing=true`, report the individually atomic
    ready/installing/missing observations, and defer cross-component identity
    comparison; do not manufacture a mismatch from a changing pair. This
    status read never waits. `syncing` is an independent probe of the
    provisioning lock.
  - In a coherent idle snapshot, unsafe/corrupt state or two ready components
    whose bound SNV identities do not agree exits 1 with the exact top-level
    envelope
    `{status:"error",code:"ASSET_STATUS_INVALID",message:"installed asset state is invalid",details:{snv:<observation>,runtime:<observation>}}`.
    For identity mismatch, the SNV observation stays ready and runtime becomes
    `{status:"error",code:"PROFILE_INCOMPATIBLE",message:"installed runtime profile is bound to a different SNV bundle"}`.
    Other component errors retain their existing mapped code/message inside
    the same ordered details object. Invalid state is never reported as
    missing or ready.
- Fully successful sync output is exactly the serialized object shape
  `{status:"ready",snv:<existing SyncOutcome>,runtime:<existing RuntimeSyncOutcome>,downloaded_bytes,resumed_bytes}` in that order. The existing nested
  field orders remain unchanged.
- Define failure wrapping at the operation boundary. UTF-8/CLI parsing, path
  resolution, platform rejection, provisioning-lock acquisition, and checked
  aggregate-counter overflow are top-level failures using the ordinary
  `{status:"error",code,message,details:null}` envelope. Path failures exit 2;
  the others exit 1. Aggregate overflow uses `ASSET_STATE_INVALID` and message
  `asset byte counter overflow`. Only an error returned after at least one
  component attempt is wrapped as `ASSET_SYNC_INCOMPLETE`.
- Remove parsing, help, specs, and documentation for `pangopup assets sync`,
  `pangopup assets status`, and `pangopup assets runtime status`; they become
  ordinary `CLI_USAGE` errors with no compatibility alias. Retain
  `pangopup assets install` and `pangopup assets runtime install` for explicit
  local/offline input.
- Keep normal tests miniature and offline. Do not download production assets,
  rebuild data, run production ONNX inference, add persistent progress,
  change transport/index formats, add aliases, HTTP, installer/release
  packaging, Docker/systemd, ARM/macOS support, repair/GC/rollback, or publish
  anything.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/delivery.md`, `architecture/runtime-data.md`,
  `architecture/service.md`, `planning/frontier.md`, and `planning/faq.md`.

## Success Checklist

- Injected miniature component runners prove exact byte-for-byte successful
  JSON, checked aggregate counters, SNV-before-runtime ordering, online
  dependency short-circuit, partial-success preservation, and exact nested
  error JSON.
- Offline tests prove a complete pair installs/reuses without network and an
  empty pair reports both bounded missing inventories in one failure.
- Concurrency tests prove one top-level owner, one immediate `ASSET_LOCKED`
  loser with zero component calls, `syncing=true` while held, crash/return
  release, and uninterrupted lookup of the last active bundle.
- Status tests cover ready, partial in both directions, missing, installing,
  syncing, a coherent idle snapshot, an install that begins or ends around the
  read, compatible identity, mismatched identity, and corrupt/unsafe component
  state with exact JSON and exit behavior.
- Executable specs prove the new grammar and rejection of all three removed
  nested status/sync forms. Existing explicit install and lookup behavior
  remains byte-compatible.
- Normal tests use no GitHub network, production payload, full score scan,
  asset rebuild, or production model initialization.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **One command or expose two asset families.** Users need a usable splice
   runtime, not delivery-layer knowledge. Compose the existing typed operations
   and retain their component results inside one response.
2. **Rollback or retain partial success.** Rolling back a valid multi-gigabyte
   SNV installation adds I/O and destroys reusable progress. Retain it, return
   nonzero with both outcomes, and make retry idempotent.
3. **Wait or fail fast under concurrency.** A download may be long and CLI
   callers need bounded behavior. Hold one nonblocking orchestration lock; the
   loser exits immediately and status exposes the active operation.
4. **Progress database or lock observation.** Exact byte progress would add a
   new durable protocol. Report only `syncing` and the two atomically visible
   installed states; persistent progress remains out of scope.
5. **Compatibility aliases or closed grammar.** The old commands expose
   implementation history and have not been released in an executable
   distribution. Remove them now rather than carry aliases indefinitely.

## Dependencies

- Ticket 035, complete: pinned runtime transport sync and direct atomic
  installation.
- Existing pinned SNV sync, shared installation lock/status, and activated
  lookup/model routing.

## Notes

- The public release profiles remain compile-time authorities. Never query a
  moving `latest` release or accept operator URLs.
- The provisioning lock belongs in the secure Linux data-root boundary and
  follows the same no-follow, ownership, type, and mode validation as existing
  asset locks. Merely seeing its pathname is not evidence that it is held.
- JSON object ordering is part of the executable acceptance bytes even though
  JSON consumers should not rely on order.
- Illustrative placeholders in the scope denote the existing typed string/path
  values; tests freeze literal representative byte strings for every stated
  shape. Preflight and lock errors always carry `details:null`.

## Coordinator Authorship

Coordinator: `/root`

Drafted from the shipped Ticket 035 primitives and the accepted user-facing
composition plan. The coordinator does not implement or approve this ticket.

## Independent Ticket Review

Reviewer: pending

First review: REJECT.

- Offline aggregation could not safely invoke runtime installation without an
  active SNV and did not define complete/incomplete/corrupt cache outcomes.
- Exact component/status/error JSON was underspecified.
- Independent status reads could observe two different installation moments
  and falsely report an identity mismatch.
- The boundary between top-level failures and wrapped component failures was
  ambiguous.

The coordinator added a bounded non-installing runtime-cache inspection,
literal component/error shapes, a coherent idle snapshot under the shared
install lock with explicit in-progress behavior, and exact failure wrapping.
The same reviewer must re-review this revision.

Final re-review: ACCEPT. The reviewer confirmed that all four findings are
resolved, the support primitives remain bounded to the combined provisioning
outcome, lock order remains compatible, and the acceptance tests can prove the
contract without production downloads or model initialization.

## Implementation Evidence

Developer: `/root/ticket036_implementation`

- Added typed `pangopup-assets` whole-product sync/status outcomes, the
  data-root provisioning lock, coherent shared-lock status reads, and bounded
  runtime-cache inspection. The composition retains the existing SNV/runtime
  transport and installation primitives.
- Replaced the three historical nested sync/status commands with exact
  top-level `pangopup sync` and `pangopup status` grammar and ordered compact
  success/error JSON. Explicit SNV/runtime install commands remain available.
- Added offline miniature tests for ordering, totals/overflow, dependency
  short-circuit, both missing inventories, partial outcomes, lock exclusion
  and release, syncing/installing observations, ready/partial/missing identity
  policy, mismatch/corruption policy, and exact CLI bytes. Updated executable
  specs and every named durable/user document.
- Focused evidence: `cargo test -p pangopup-assets provisioning
  --no-fail-fast` passed 8; the read-only malformed-cache probe and exact
  combined-status CLI tests passed independently; the complete CLI package now
  passes all 26 tests across targets.
- Code-review remediation replaced recovery-capable published-transport lookup
  in the offline inspection path with a separate existing-path-only cache
  opener, existing nonblocking lock, and read-only closed-inventory probe. An
  adversarial 100-extra-entry cache test proves the probe stops at its first
  unexpected entry and leaves every entry untouched. Additional accepted-ticket
  tests prove zero component calls for a top-level lock loser while a real
  installed lookup remains usable, a complete cached pair through locked
  top-level composition, and byte-exact ready/mismatch/corrupt CLI status
  envelopes and exits.
- Developer gate evidence: `make lint` passed (including cargo-deny, with only
  the repository's established duplicate-dependency warnings), `make test`
  passed the full workspace suite, and `make spec` passed 233 with 6 skipped.
- No production asset, network request, model initialization, format change,
  publication, or external effect was used.

## Adversarial Code Review

Reviewer: `/root/ticket036_code_review`

First review: REJECT.

- The first offline runtime-cache inspector reused a recovery-capable helper
  that could delete an unbounded unexpected directory inventory, contradicting
  the read-only bounded contract and repair/GC exclusion.
- Top-level lock exclusion with usable lookup, complete cached-pair
  composition, and exact ready/mismatch/corrupt CLI status tests were missing.

The developer added a separate existing-path-only read-only cache opener and
fixed-inventory probe, plus the missing top-level and byte-exact tests. The
100-extra-entry adversarial case proves early stop and an unchanged directory.

Final re-review: ACCEPT. The reviewer verified both remediations, ran all eight
provisioning tests plus the focused cache/status cases, found no regression,
and confirmed `git diff --check` is clean.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: `/root`

- Independently inspected the final diff, review remediation, working-tree
  inventory, and current-state documentation. Remaining old nested commands
  occur only in executable rejection examples; shipped/future claims agree.
- `git diff --check` passed.
- `make lint` passed; cargo-deny reported only the repository's established
  duplicate-dependency warnings and all advisory/bans/license/source policies
  passed.
- `make test` passed the full workspace, including 91 asset tests and 26 CLI
  package tests; only the six established maintainer/production measurements
  remain ignored.
- `make spec` passed 233 with 6 skipped.
- No external effect or production asset access occurred. Ticket 036 is
  complete and the next bounded outcome is Linux x86_64 executable packaging
  and the binary-only installer, not HTTP or publication yet.
