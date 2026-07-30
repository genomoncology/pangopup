# 031 — Remove the custom release uploader

Status: complete

## Why

Pangopup contains a coordinator-only `pangopup-build release upload-asset`
command with roughly three thousand lines of upload supervision and integration
tests. An interrupted test left helper and fake-upload descendants alive under
PID 1. The command is not used by lookup, inference, installation, or remote
sync; it only wraps the official GitHub CLI for a maintainer publication
operation.

Repairing and retaining a second process supervisor would add product code and
failure modes without improving the runtime. The next public asset upload is
blocked until this lifecycle defect is removed or fully corrected. The smaller
and safer outcome is to remove the custom uploader and have the coordinator use
the official `gh release` interface directly after Pangopup's deterministic
local preparation and verification have succeeded.

## Scope

- Remove the `release upload-asset` command, its public/test API, Linux
  process/lease/signal supervision, fake-`gh` fixtures, and uploader-only tests.
- Preserve only the inert legacy `AssetErrorKind::ReleaseUpload` enum/code
  spelling because the established SNV builder v1 fingerprint hashes the shared
  error vocabulary wholesale. Nothing may construct it from an upload path.
  Do not redesign builder provenance or churn its identity in this deletion.
- Keep `pangopup-build release prepare`, the checked SNV release profiles, and
  all deterministic local transport verification unchanged.
- Make root, namespace, and leaf help describe only the remaining supported
  maintenance commands. Treat removal of this never-runtime, maintainer-only
  command as an intentional breaking maintenance-interface change; do not add
  an alias, deprecated shim, or restore flag.
- Replace documentation of the custom command with the plain publication
  boundary: the coordinator invokes an authenticated official `gh` executable
  through a later reviewed draft-first publication lifecycle. Deletion
  intentionally removes the uploader's sealed-small-file and large-file lease
  guarantees; deterministic preparation alone does not make a local pathname
  immutable. The exact stable-source policy, publication commands, and
  evidence belong to the later asset-publication ticket, not this removal.
- Close
  `planning/issues/2026-07-24-release-process-lifecycle.md` by recording that
  the risky mechanism was deleted rather than repaired.
- Update these current behavior surfaces in the same diff:
  `README.md`, `architecture/delivery.md`, `planning/faq.md`,
  `planning/frontier.md`, `planning/issues/2026-07-24-release-process-lifecycle.md`,
  `spec/build-cli.md`, and `spec/snv-release.md`.
- Do not change GitHub settings, contact GitHub, create or alter a release,
  upload an asset, rebuild or open a production asset, implement runtime-asset
  sync, or change scoring/runtime behavior.

## Success Checklist

- `pangopup-build release --help`, root help, and the complete catalog no longer
  advertise `upload-asset`; `release prepare` remains present and unchanged.
- An attempted `pangopup-build release upload-asset` is rejected before any
  filesystem or process side effect through the ordinary closed command
  grammar. `make spec` proves both the removed command and retained
  `release prepare` behavior.
- No production or test code can spawn `gh`, hold a large release member under
  a lease, install signal handlers for an upload, or create uploader helper
  descendants. Focused tests prove the remaining release preparation API and
  catalog; an absence check covers the removed Rust module and exported API.
- The uploader-only subprocess, descriptor, signal, deadline, lease, and
  orphan-process tests and fixtures are removed rather than replaced with
  another supervisor.
- Current documentation clearly distinguishes deterministic local preparation
  (shipped) from direct coordinator use of official `gh` (future public
  effect), contains no claim that Pangopup ships an uploader, and does not
  describe prepared local pathnames as immutable.
- The release-process lifecycle issue records why deletion fully removes its
  observed leak and descendant risk. It does not claim that public release
  publication or the separate repository-security baseline is complete.
- The diff does not change the checked SNV release profile/proof bytes, runtime
  transport bytes, model/reference/mask identities, CLI scoring output, or
  dependency lockfile except where removing now-unused uploader-only
  dependencies makes a lockfile change mechanically necessary.
- The established hard SNV and reference builder fingerprints remain unchanged.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

### Delete the mechanism instead of repairing it

- **Consideration:** The current wrapper tries to make a maintainer's `gh`
  upload crash-safe using Linux process groups, parent-death handling,
  signalfd, file leases, and extensive test hooks, but it has itself produced
  orphan processes.
- **Options:** Repair and extend the supervisor; replace it with a new
  watchdog; or remove the wrapper and use official `gh` directly.
- **Trade-offs:** Repair preserves a specialized content-blind upload path but
  keeps a large security-sensitive subsystem. Direct `gh` gives up that custom
  in-process lease policy but deletes the defect and relies on the maintained
  publication tool already required by the coordinator.
- **Decision:** Delete the wrapper. Pangopup owns deterministic asset
  preparation and verification; GitHub's CLI owns transport to GitHub.

### Keep publication outside the shipped binary

- **Consideration:** Uploading is an authenticated public mutation performed
  once by a maintainer, while Pangopup's installed runtime must remain
  credential-free and deterministic.
- **Options:** Move upload into another Pangopup binary/library; shell out from
  the runtime; add a workflow that publishes automatically; or keep the action
  as an explicit coordinator `gh` operation.
- **Trade-offs:** Another wrapper recreates the same ownership problem.
  Runtime or automatic publication broadens credential and trigger risk.
  Explicit coordinator operation is less automated but easiest to audit.
- **Decision:** No Pangopup crate or binary uploads releases. A later reviewed
  publication ticket will name the exact `gh` commands and perform them only
  after its commit and remote gate are green.

### Preserve local proof and verify the remote result

- **Consideration:** Removing the wrapper must not reduce confidence that the
  uploaded bytes are the reviewed bytes. The deleted small-file snapshots and
  large-file lease were the only in-process stability mechanism; deterministic
  preparation does not freeze a pathname against later mutation.
- **Options:** Trust command success; retain the custom read lease; or compare
  a controlled stable source and prepared manifest with GitHub's observed
  release metadata and server-side digests before making the release public.
- **Trade-offs:** Command success alone is weak. The lease is complex and only
  protects the local pathname during one process. A draft-first lifecycle keeps
  a bad or partial upload non-public and correctable while end-to-end comparison
  proves what GitHub actually received; immutable finalization then prevents
  replacement.
- **Decision:** Retain deterministic local preparation unchanged. The later
  publication ticket must define and prove a simple controlled stable-source
  policy, upload the exact closed inventory to a non-public mutable draft,
  compare GitHub names, sizes, digests, and target commit while correction or
  deletion remains possible, and only then publish/finalize and verify
  `immutable=true`. If GitHub's actual immutable-release semantics do not
  support that exact order, that ticket must prove a safe equivalent before
  any public effect.

### Make the maintenance-interface break explicit

- **Consideration:** `upload-asset` appears in public help and documentation,
  but it is a coordinator-only maintenance command rather than a user/runtime
  contract.
- **Options:** Preserve a compatibility shim; hide but retain it; or remove it
  from code, help, tests, and current documentation.
- **Trade-offs:** A shim preserves command recognition but leaves dead policy
  and invites continued use. Full removal is a visible maintenance break but
  prevents accidental reliance on a rejected design.
- **Decision:** Remove it without a restore flag. The issue resolution and
  current documentation are the compatibility ledger: deterministic
  `release prepare` remains supported, and publication moves to official
  `gh`.

### Retain the two-line error-vocabulary tombstone

- **Consideration:** The v1 SNV builder fingerprint intentionally remains
  immutable, but its established causal inventory hashes the complete shared
  `error.rs` vocabulary. Removing the unused `ReleaseUpload` enum/code spelling
  changes that fingerprint even though no SNV construction behavior changed.
- **Options:** Bless a false new SNV builder identity; redesign provenance in
  this ticket; or retain the inert spelling while deleting every command,
  constructor, supervisor, and upload behavior.
- **Trade-offs:** The inert spelling is small legacy vocabulary. A provenance
  redesign is materially broader; changing the hard identity falsely reports a
  data-builder change.
- **Decision:** Retain the unreachable enum/code spelling as a provenance and
  public-vocabulary tombstone. It does not preserve the removed command or any
  upload capability. A future provenance version may remove it deliberately.

## Dependencies

- Ticket 030 is complete: the exact derived runtime transport exists locally
  and no publication must rebuild it.

## Notes

- The developer must inspect shared release code before deleting: preparation
  types, canonical release-profile parsing, checked production profile/proof,
  and `prepare_release` stay. Delete only upload-specific APIs and helpers.
- Likely owning files include
  `crates/pangopup-assets/src/release_upload_linux.rs`,
  uploader-specific sections of `crates/pangopup-assets/src/release.rs`,
  exports in `crates/pangopup-assets/src/lib.rs`, dispatch/catalog entries in
  `crates/pangopup-build/src/{main.rs,cli.rs}`, and uploader-only sections of
  `crates/pangopup-build/tests/transport.rs`. Check the actual tree rather than
  assuming this list is exhaustive.
- The exact repository gate is `make lint`, `make test`, and `make spec`;
  there is no `make check`.
- Normal gates must not use the network, invoke real `gh`, open production
  assets, or leave child processes. Public-repository hygiene forbids secrets,
  credentials, machine-specific absolute paths, or generated production
  payloads.
- Evidence and local paths in this ticket are illustrative; do not commit
  scratch output.

## Coordinator Authorship

Coordinator: Codex `/root`, 2026-07-30

The coordinator drafted this ticket from Ticket 030's shipped local runtime
transport, the open release-process issue, and the rolling publication
frontier. It does not implement or approve the product-code diff.

## Independent Ticket Review

Reviewer: `/root/ticket031_design_review`

First review: REJECT. The ticket incorrectly described the prepared local
pathnames as immutable and replaced the deleted uploader's snapshot/lease
guarantee with only after-the-fact digest comparison. The coordinator accepted
the finding. The revision now states that deletion intentionally removes that
stability mechanism and requires the later public-effect ticket to prove a
controlled stable-source, draft-first upload, remote inventory/digest check,
then immutable finalization (or a proven safe equivalent supported by GitHub).

Re-review: ACCEPT. The reviewer found no remaining material issue. The revised
ticket keeps deletion bounded, preserves release preparation, states the
removed stability guarantee honestly, and leaves the safe draft-first public
effect to a later independently reviewed ticket.

Coordinator final-gate scope revision: the broad test proved that deleting the
shared `ReleaseUpload` error spelling changed the hard SNV v1 builder
fingerprint even though release code is otherwise excluded. The ticket now
explicitly retains only that inert two-line vocabulary tombstone instead of
blessing a false builder change or expanding into a provenance redesign.
Ticket re-review: ACCEPT. The same reviewer accepted the narrow revision. The
legacy enum spelling and code arm preserve the established SNV v1 provenance
preimage but restore no upload constructor, command, process path, or supported
maintenance behavior.

## Implementation Evidence

Developer: Codex `/root/ticket031_implementation`, 2026-07-30

- Removed the `release upload-asset` catalog/dispatch path, public and injected
  upload APIs, Linux upload supervisor module, and uploader-only fake-process
  helpers and tests. Retained only the inert legacy `ReleaseUpload` enum/code
  vocabulary required by the established SNV v1 fingerprint; no constructor or
  upload behavior remains. Release preparation types, parsing, checked
  production contract, and miniature preparation tests remain.
- Updated every named current-behavior document and executable spec. The
  lifecycle issue is closed by deletion while later direct official-`gh`
  publication and the separate repository-security baseline remain unfinished.
- Focused checks passed: three `pangopup-assets` release contract unit tests,
  two retained `pangopup-build` release preparation integration tests, six
  command-catalog unit tests, focused Clippy for both owning packages with
  warnings denied, and 27 changed-spec blocks (`build-cli.md` plus
  `snv-release.md`; the final release-spec rerun passed 12/12).
- After the final-gate scope revision, all 17 source-fingerprint unit controls,
  all four builder-provenance integration controls, the exact SNV transport
  builder-identity regression, and all 27 changed executable spec blocks
  passed. The hard SNV v1 identity is again the established
  `b3bdc4d9d8e710fb554fd47f0cfc6f6a7bb764451069e6ae4a98534d8c5dc6a2`;
  no expected digest or fixture was changed.
- The checked release profile remains
  `sha256:63f3842ea6cb40ebc0a2b6ca23fba4f35d53f829d96c33f597a2c5bcac238ca6`;
  its proof remains
  `sha256:9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475`.
  `Cargo.lock` is unchanged. No production asset was opened and no network,
  GitHub, release, or upload operation was performed.

## Adversarial Code Review

Reviewer: Codex `/root/ticket031_code_review`

First review: REJECT. One medium documentation finding remained:
`planning/frontier.md` still said release-ready asset publication must close
the release-process blocker even though this ticket closes that issue by
deleting the mechanism.

Developer remediation: the asset-readiness boundary now consistently requires
the separate repository-security blocker and the later reviewed safe
publication lifecycle. It no longer describes the deleted uploader issue as
open. `git diff --check`, the focused current-document catalog unit test, and
all 27 `build-cli.md`/`snv-release.md` executable spec blocks passed after the
change.

Coordinator final-gate finding: the first full `make test` observed SNV builder
fingerprint `69c1…` instead of hard identity `b3bd…` because deletion of the
legacy `ReleaseUpload` error spelling changed the wholesale shared-error
preimage. The coordinator returned the material scope conflict to the same
ticket reviewer rather than blessing a false builder change.

Developer remediation: after the revised ticket was accepted, restored only
the inert enum spelling and `RELEASE_UPLOAD` code arm. The executable absence
spec permits exactly those two tokens while still rejecting every uploader
module, function, outcome type, and constructor use. Focused fingerprint and
changed-spec evidence is recorded in the implementation evidence above.
Re-review: ACCEPT. The same reviewer confirmed that no uploader constructor,
module, API, command, supervisor, or behavior remains; the absence spec permits
exactly the inert vocabulary tombstone; and all hard identities remain
unchanged. No material finding remains.

## External Effect Evidence

Coordinator: not applicable. This ticket explicitly performs no GitHub
mutation or asset upload.

## Coordinator Final Check

Coordinator: Codex `/root`, 2026-07-30

- `make lint`: passed.
- `make test`: passed across the workspace; four production/measurement tests
  remained intentionally ignored.
- `make spec`: 218 passed, 4 skipped.
- `git diff --check`: passed.
- Current-state documentation was scanned for stale uploader and blocker
  claims. Only the intentional removed-command executable check and exact
  absence/tombstone check remain.
- No network, GitHub setting, release, upload, or production-asset operation
  occurred.
