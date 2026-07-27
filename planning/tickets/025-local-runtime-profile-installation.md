# 025 — Install and atomically select one local runtime profile

Status: ready

## Why

Ticket 024 created the small canonical authority that says which SNV index,
model, GRCh38 sequence bundle, mask, and scoring policy belong together.
Pangopup still cannot install that tuple: only the SNV transport has an XDG
installer, while model fallback still requires three unrelated paths.

The next bounded step is a local, offline installer. It reuses the already
installed and certified SNV bundle, copies the three derived fallback assets
into one private immutable XDG store, and changes one coherent profile pointer
only after every byte and receipt is durable. Runtime lookup discovery,
network delivery, and publication remain later work.

## Scope

- Extend `pangopup-assets`; do not add another crate or database. Linux remains
  the supported local-install platform.
- Add these CLI commands:

  ```text
  pangopup assets runtime install \
    --profile <CANONICAL_PROFILE_JSON> \
    --model-bundle <DIR> \
    --reference-bundle <DIR> \
    --mask <FILE> \
    [--data-dir <ABSOLUTE_PATH>]

  pangopup assets runtime status [--data-dir <ABSOLUTE_PATH>]
  ```

  Keep the existing SNV `assets install`, `assets sync`, `assets status`, and
  lookup behavior byte-for-byte compatible.
- Require an existing active SNV installation under the selected XDG data
  root. Add a bounded active-SNV admission API that validates the canonical
  installed receipt and manifest, exact notice, score member identity/size,
  immutable wrapper state, and held member descriptors without mmap-opening or
  reading `scores.pgi`. Require those facts to equal the Ticket 024 profile.
  Never decompress, copy, hash, mmap, or recertify the 15 GB SNV payload here.
- Parse and trust-check the exact canonical Ticket 024 profile before opening
  fallback sources. Source files need only be held readable regular files with
  one link and no symlink; do not impose installed ownership/modes on them.
  The retained inputs legitimately include modes `0664`, `0775`, and `0400`.
  Stream each held model/reference/mask descriptor exactly once into private
  staging with fixed byte ceilings while hashing it, compare digest/length to
  the profile, then perform bounded structural/manifest validation against the
  staged copy. Do not fully hash a 772 MB source and then reopen/copy it.
  Source pathname mutation may at most make installation fail; it must never
  publish mixed bytes.
- Use this versioned layout below the existing resolved data root:

  ```text
  runtime/
    active.json
    components/
      model/<64-hex-bundle-id>/bundle/{manifest.json,NOTICE,model.onnx}
      reference/<64-hex-bundle-id>/bundle/{manifest.json,NOTICE,reference.pgr}
      mask/<64-hex-member-id>/domains.pgm
    profiles/<64-hex-profile-id>/{profile.json,receipt.json}
    .staging/<private-nonce>/...
  ```

  `active.json` contains only schema
  `pangopup.runtime-active.v1` and the canonical profile ID. The immutable
  profile receipt contains schema `pangopup.runtime-install-receipt.v1`, the
  profile ID, the active SNV bundle ID, and relative component member
  paths/sizes/SHA-256 values. It contains no source paths, URLs, timestamps,
  hostnames, credentials, or mutable aliases.
- Use the existing resolved XDG root and ownership rules. `runtime`,
  `components`, `profiles`, and `.staging` are private `0700` while mutable;
  installed component/profile files are `0444`, immutable wrappers are `0555`,
  `active.json` is `0600`, and the lock is private. Installed state additionally
  rejects wrong ownership/modes. All state rejects symlinks, non-regular
  members, multiply linked files, non-UTF-8 fixed entry names, extra entries,
  traversal, cross-device staging, and identity/path conflicts.
- Reuse the existing root `.install.lock` for both SNV and runtime installation.
  One nonblocking exclusive lock therefore owns SNV/runtime reconciliation,
  installation, reuse, and activation, and the active SNV cannot change between
  admission and coherent-profile commit. Status probes that same lock without
  waiting. Lookup and existing SNV status remain lock-free.
- Publish each new immutable component/profile directory with no-replace
  descriptor-relative renames and directory fsyncs. Only after all referenced
  immutable objects validate does a same-directory staged `active.json`
  atomically replace the old runtime pointer followed by root-directory fsync.
  A failure before that commit point leaves the prior coherent profile active;
  cleanup/reconciliation must never follow hostile entries.
- Idempotent reinstall of the exact tuple reports `reused` and writes no
  component bytes. An immutable identity collision is a stable conflict and
  never overwrites bytes. Reducing or changing the accepted tuple is outside
  this v1 ticket.
- Freeze the observable compact JSON below in this field order with one final
  LF. The `installed|reused` notation means the literal matching status:

  ```text
  install:
  {"status":"installed|reused","profile_id":"sha256:…","snv_bundle_id":"sha256:…","model_bundle_id":"sha256:…","reference_bundle_id":"sha256:…","mask_sha256":"sha256:…"}

  status missing:
  {"status":"missing","data_dir":"<absolute data root>"}

  status installing with no active runtime profile:
  {"status":"installing","data_dir":"<absolute data root>"}

  status ready:
  {"status":"ready","profile_id":"sha256:…","snv_bundle_id":"sha256:…","model_bundle_id":"sha256:…","reference_bundle_id":"sha256:…","mask_sha256":"sha256:…","model_path":"runtime/components/model/<hex>/bundle","reference_path":"runtime/components/reference/<hex>/bundle","mask_path":"runtime/components/mask/<hex>/domains.pgm","installing":false}
  ```

  A ready profile remains `ready` with `installing:true` while the shared lock
  is held. Pin stable error codes: existing `CLI_USAGE`, `PATH_INVALID`,
  `ASSETS_MISSING`, `ASSET_STATE_INVALID`, and `ASSET_LOCKED`, plus
  `PROFILE_INCOMPATIBLE`, `PROFILE_UNSAFE`, `PROFILE_CORRUPT`, `INPUT_IO`,
  `OUTPUT_IO`, and `INSTALL_CONFLICT`. Messages are fixed and redacted.
- `assets runtime status` performs bounded canonical metadata and
  member-shape/size checks only—no network, full member hashing, mmap payload
  scan, model initialization, or inference. Malformed active state is a stable
  redacted error, not `missing`.
- Add inside-out tests with checked miniature files for:
  exact layout/receipt/active JSON, first install, idempotent zero-copy reuse,
  nonblocking lock/status, prior-active preservation, staged-copy corruption,
  source replacement/truncation, symlinks/hardlinks/extra members/modes,
  component/profile identity collision, malformed active/receipt, transition
  failures at staged-object durability, component publication, profile
  publication, before active rename, and after rename before root fsync,
  orphan-stage reconciliation, read-only sources, no partial final objects, and
  no SNV payload/model-session reads. Do not duplicate every chmod/fsync point.
  Use fixed-size test assets; no production-size copy or long validator enters
  normal gates.
- Successful miniature installation uses an injected crate-private trusted
  profile contract in Rust integration tests; there is no shipped synthetic
  profile or compatibility bypass. Executable specs cover CLI grammar,
  missing/malformed active SNV/runtime state, shared-lock status semantics,
  exact stable JSON/errors, and absence of partial output. The successful real
  production-only CLI proof remains deferred until derived transports exist.
- Retain one small deterministic installation receipt/layout artifact from
  miniature inputs in
  `planning/artifacts/025-local-runtime-profile-installation.md`. Do not perform
  a production 15 GB SNV installation or duplicate retained production assets
  in this ticket.
- Update `AGENTS.md`, `README.md`, `architecture/README.md`,
  `architecture/runtime-data.md`, `architecture/service.md`,
  `planning/faq.md`, and `planning/frontier.md`. Add one ADR. State plainly
  that the tuple can be installed and inspected locally but lookup does not yet
  discover/use it, and network sync/publication/HTTP/deployment remain future.

Explicit exclusions: raw-source rebuild, SNV transport install/sync changes,
copying the 15 GB SNV payload, production large-asset installation, lookup
runtime-profile discovery, automatic fallback, model-cache relocation,
rollback/GC/repair commands, networking, GitHub release assets, publication,
progress persistence, HTTP, status HTTP endpoint, service lifecycle, Docker,
systemd, signing, SBOM, GPU/MPS/CUDA, and public external effects.

## Success Checklist

- From an existing active miniature SNV installation, one command durably
  installs exact model/reference/mask bytes and atomically selects the complete
  coherent profile.
- No failure, crash window, hostile path, concurrent installer, or immutable
  conflict exposes a partial/mixed profile or replaces the prior active tuple.
- Exact reinstall performs zero component copy writes and reports `reused`.
- Status is bounded, lock-aware, offline, redacted, and distinguishes missing,
  installing, ready, and malformed state.
- The existing SNV commands and lookup behavior remain byte-exact.
- Fast unit/integration/spec coverage proves the lifecycle without copying
  production-size assets or retaining a long-running verifier.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Reuse the certified installed SNV object.** Copying or rehashing 15 GB
   would repeat work the existing receipt already proves. The coherent profile
   references that immutable installed object by identity.
2. **Copy the three fallback assets into Pangopup-owned immutable storage.**
   Referencing arbitrary source paths would make activation mutable and
   host-specific. Hardlinks would let source mutation alter installed bytes;
   bounded streaming copies plus destination authentication provide ownership
   and isolation.
3. **Keep a separate runtime active pointer.** Existing root `active.json`
   continues to mean the public SNV lookup bundle. `runtime/active.json` commits
   the four-asset tuple without silently changing lookup behavior before the
   next runtime-discovery ticket.
4. **Commit only after all immutable objects are durable.** Components and the
   profile may safely exist before activation; the single active rename is the
   coherence boundary. A failed install never rolls forward part of a tuple.
5. **Use files and receipts, not SQLite.** Installation state is a small
   immutable filesystem graph with one pointer. A database adds no query value
   and complicates crash recovery.
6. **Share the existing installation lock.** A separate runtime lock would let
   the active SNV bundle change between admission and runtime activation. The
   root `.install.lock` serializes both installers and removes that race without
   lock ordering.

## Dependencies

Tickets 006, 008, 024.

## Notes

- The exact production profile is
  `planning/artifacts/024-four-asset-runtime-profile.json`, but normal tests use
  miniature typed facts and files.
- Reuse the existing local installer’s descriptor-relative Linux helpers and
  fault-injection conventions where they fit; do not fork a second generic
  framework merely to avoid a focused runtime-profile module.
- Existing retained source modes are not installation-policy violations.
  Strict owner/mode enforcement begins only inside Pangopup's data root.
- This ticket intentionally does not create another local copy of the retained
  15 GB production SNV bundle or 772 MB reference bundle. Production
  clean-machine installation follows when derived release transports exist.

## Coordinator Authorship

Coordinator: Codex `/root`

Drafted after Ticket 024 froze the exact four-asset authority. This is the
first installation slice: immutable local storage and coherent selection only.
Runtime consumption and public delivery remain separate reviewed outcomes.

## Independent Ticket Review

Reviewer: Codex sub-agent `/root/ticket025_design_review`

Initial verdict: **REJECT**. The reviewer found that a separate runtime lock
could race the SNV installer, source-mode requirements contradicted retained
inputs, pre-authenticate/reopen/copy could duplicate the 772 MB reference read
and blur the descriptor boundary, active SNV admission could touch the score
mmap, a successful miniature production CLI spec was impossible, observable
JSON/errors were not frozen, and an every-fsync fault matrix was excessive.

The coordinator switched to the existing root lock, required bounded
receipt/manifest-only active-SNV admission, separated permissive held sources
from strict installed state, froze one-pass descriptor copy/hash followed by
staged structural validation, moved successful miniature installation to a
crate-private Rust test contract, pinned JSON/error behavior, and narrowed
fault injection to lifecycle transition classes.

The reviewer found one final literal mismatch: the draft used `USAGE` and
`INSTALL_LOCKED` instead of the shipped `CLI_USAGE` and `ASSET_LOCKED` codes.
The coordinator corrected both.

Revised verdict: **ACCEPT**. The reviewer confirmed the ticket is coherent,
bounded, compatible with existing contracts, and implementation-ready.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
