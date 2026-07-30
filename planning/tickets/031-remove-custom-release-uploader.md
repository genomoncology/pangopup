# 031 — Remove the custom release uploader

Status: ready

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

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable. This ticket explicitly performs no GitHub
mutation or asset upload.

## Coordinator Final Check

Coordinator: pending
