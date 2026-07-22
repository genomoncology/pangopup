# 006 — Linux XDG local installation and verified reuse

Status: proposed

## Why

After Ticket 005 ships deterministic split transport, Pangopup needs a safe
local installer before adding downloads. This slice proves Linux/XDG path
resolution, locks, exhaustive first installation, immutable publication, cheap
trusted reuse, explicit full verification, and offline/container prefetch.

It deliberately has no cache, network, automatic lookup discovery, or mutable
"current" pointer. It remains dependency-gated until Ticket 005 actually ships.

## Scope

- Extend Ticket 005's `pangopup-assets` crate with Linux installation and
  discovery. Reuse its manifest, transport verification, extraction, and
  errors; do not duplicate those paths. Keep side effects out of
  `pangopup-core`, `pangopup-index`, and `ScoreProvider`.
- Linux is the only operationally supported platform in this ticket. Use real
  Linux directory handles, `openat`/`O_NOFOLLOW` (or stronger beneath/no-symlink
  resolution), advisory `flock`, `renameat2(RENAME_NOREPLACE)`, and required
  file/directory `fsync`. Other-platform path computation may have unit tests,
  but no macOS/Windows locking, durability, or atomicity claim ships.
- Resolve the data root in exact order:
  1. explicit `--data-dir`, which must be a nonempty absolute UTF-8 path;
  2. `PANGOPUP_DATA_DIR`, which if present must be nonempty, absolute UTF-8 or
     fails `PATH_INVALID` (it never silently falls through);
  3. nonempty absolute UTF-8 `XDG_DATA_HOME`, otherwise ignore empty/relative
     XDG per its specification and try HOME;
  4. nonempty absolute UTF-8 `HOME` plus `.local/share/pangopup`;
  5. otherwise fail `PATH_UNAVAILABLE`.
  Only the `XDG_DATA_HOME` and HOME-derived defaults append `pangopup`.
  `--data-dir` and `PANGOPUP_DATA_DIR` already name the complete root.
  Non-Unicode relevant environment values fail
  `PATH_INVALID`. No path becomes relative/current-directory implicitly.
- Trust boundary: an explicit/environment/default base may exist only as a real
  directory owned by the effective uid and not group/world writable, or the
  manager creates it and its `pangopup` descendants mode `0700`. Reject
  symlinked/non-directory/untrusted base components. Beneath the opened trusted
  base dirfd, every lookup/open/create/rename is relative, no-follow, and cannot
  escape via rename/symlink races. Tests use real temporary Linux filesystems;
  inject only failpoints and a monotonic clock, not a fake filesystem. A
  root-owned read-only container mount is not an install root in this slice;
  use its verified bundle through explicit `--bundle`, or preinstall as the
  runtime euid.
- Exact installed layout, where `<B>` is the 64 lowercase hex from canonical
  installed `bundle_id` (prefix stripped):

  ```text
  <data>/bundles/<B>/bundle/{NOTICE,manifest.json,scores.pgi}
  <data>/bundles/<B>/receipt.json
  <data>/.locks/<B>.lock
  <data>/.staging/<B>/<nonce>/{marker.json,payload/...}
  ```

  Installed data and the lock are keyed only by `bundle_id`; a repackaged
  `transport_id` never duplicates 15 GB or acquires a different destination
  lock. There is no mutable current pointer and no optional selection.
- Add exact commands:

  ```text
  pangopup assets install --manifest <LOCAL_TRANSPORT_JSON>
    [--data-dir <ABS_DIR>] [--lock-timeout-seconds <0..3600>]
  pangopup assets path --bundle-id sha256:<64-lower-hex>
    [--data-dir <ABS_DIR>]
  pangopup assets verify --bundle-id sha256:<64-lower-hex> [--full]
    [--data-dir <ABS_DIR>]
  ```

  `install` always requires one local regular Ticket 005 manifest and returns
  its exact bundle ID/path. Declared parts are required and opened only when the
  final bundle is absent; cheap reuse requires no part files. `path` and
  `verify` always require a bundle ID. `pangopup lookup --bundle` is unchanged.
- Success is compact, LF-terminated JSON, empty stderr, exit 0, exact key order:
  - install: `status` (`installed` or `reused`), bundle_id,
    requested_transport_id, installed_from_transport_id, path;
  - path: `status=ready`, bundle_id, path;
  - verify: `status=verified`, `mode` (`cheap` or `full`), bundle_id, path.
  Failures write no stdout and the existing compact error object with closed
  codes. `CLI_USAGE`, `INVALID_ID`, `PATH_INVALID`, and `PATH_UNAVAILABLE` exit
  2. `DATA_IO`, `LOCK_TIMEOUT`, `TRANSPORT_INVALID`, `BUNDLE_INVALID`,
  `INSTALL_CONFLICT`, and `STAGING_INVALID` exit 1. Messages are noncontractual;
  `details` is exactly null in this slice; code/details and zero-stdout behavior
  are contractual.
- Lock with `flock(LOCK_EX|LOCK_NB)` on the no-follow regular lock-file handle,
  retrying against a monotonic deadline. Default timeout is 3600 seconds; 0 means
  one attempt; maximum 3600. After acquisition, write/sync diagnostic holder
  metadata (schema, bundle ID, pid, Linux process start ticks) but treat the
  kernel lock—not metadata—as ownership. Process death releases the lock; a
  stale lock file is safely reused/truncated only after acquiring it. Never
  steal a live lock. Same-bundle installers converge subject to timeout;
  different bundle IDs use different locks.
- Install ordering is exact. First boundedly parse/canonical-validate only the
  supplied transport manifest, including its self-derived transport ID, to get
  requested transport ID and bundle ID; do not read parts yet. Lock that bundle
  ID and reconcile marked stale staging. If the final bundle passes the trusted
  cheap-reuse boundary below, return `reused` without opening any part. Its
  immutable receipt's original transport remains
  `installed_from_transport_id`; the supplied manifest is reported separately
  as `requested_transport_id`, and transport mismatch is allowed when bundle ID
  matches. If final is absent, use one combined shared-library pass: stream-
  verify parts/compressed/archive/provenance while extracting once into data-root
  staging, then exhaustively verify that same staged bundle. Never call Ticket
  005 scratch `verify` and unpack a second copy.
- After first-install exhaustive verification, write this exact strict JCS
  receipt (all JSON numbers use `0..=2^53-1`; member order is fixed):

  ```text
  schema: "pangopup.install-receipt.v1"
  bundle_id: "sha256:" + 64 lowercase hex
  installed_from_transport_id: "sha256:" + 64 lowercase hex
  bundle_schema: "pangopup.bundle.v1"
  index_format: "pangopup.fixed11.v1"
  members: exactly [
    {path:"bundle/NOTICE", size:safe_json_u64, sha256:sha256},
    {path:"bundle/manifest.json", size:safe_json_u64, sha256:sha256},
    {path:"bundle/scores.pgi", size:safe_json_u64, sha256:sha256}
  ]
  ```

  Paths are resolved beneath the staged/final `<B>` dirfd. It has no
  timestamp/host field. Installed members and receipt are regular no-follow,
  uid equals euid, exact mode `0444`; lock files are regular mode `0600`;
  ownership markers are regular mode `0400`; staging wrappers/payload are mode
  `0700` until publication; final `<B>` and `bundle` directories are mode
  `0555`. Sync members, receipt, payload/wrapper and parent directories, then
  publish only the payload `<B>` directory with no-replace rename; fsync source
  and destination parent dirfds after rename; unlink the exact still-open/
  no-follow-validated `marker.json`; `rmdir` the now-empty wrapper; and fsync
  `.staging/<B>` plus the bundles parent again before success.
- If the final directory already exists, do not mutate it. Reuse only when its
  strict receipt is canonical, requested bundle ID/declared member sizes/modes
  match, the small installed `manifest.json` is rehashed and its digest equals
  bundle ID, files are
  immutable regular no-follow files, and cheap `BundleOpen` structural/
  compatibility validation succeeds. This trust model assumes a manager-owned,
  immutable publication plus receipt. Cheap reuse intentionally does not hash
  same-size `NOTICE` or ordinary payload and may not detect post-install local
  tampering; `verify --full` hashes every receipt member and runs exhaustive
  verification. A conflicting/corrupt final directory returns
  `INSTALL_CONFLICT` and is left untouched; repair/removal is deferred.
- `path` uses the same cheap receipt/open boundary. `verify --full` detects
  same-size post-install corruption. Neither command needs Ticket 005 parts.
  Clearing or moving the original local transport after installation cannot
  affect path/verify/explicit-bundle lookup.
- Staging ownership and interruption are exact. Each install creates a
  cryptographically random 128-bit lowercase-hex nonce directory with `mkdirat`
  no-replace beneath `.staging/<B>`. That wrapper contains mode-0400
  `marker.json` binding schema, bundle ID, nonce, and euid plus mode-0700
  `payload/`; extraction, `bundle/`, and `receipt.json` exist only in payload.
  Publish only payload to `bundles/<B>`, perform the post-rename fsyncs, unlink
  the validated marker, remove the then-empty wrapper, and fsync the staging
  parent, so the ownership marker never enters final content. Ordinary injected failures clean
  their wrapper. SIGKILL may leave only unpublished staging. On the
  next operation holding the same bundle lock, cleanup removes every direct real
  nonce directory owned by euid whose no-follow marker exactly matches directory
  name/bundle, whether or not a receipt was already written; `.staging` is never
  a published location, so cleanup always restarts rather than resumes. It never follows links,
  crosses mounts, or removes unrelated/malformed entries (which cause
  `STAGING_INVALID`). One crash-safe exception applies under the same bundle
  lock: an exact expected-prefix, real, same-device, euid-owned nonce wrapper
  with no marker may be removed only if no-follow dirfd enumeration proves it
  completely empty. A markerless wrapper containing any entry remains
  `STAGING_INVALID`. Published `<data>/bundles` is never cleanup input.
  SIGKILL after publish but before fsync/marker cleanup leaves a valid final plus
  marked wrapper; the next operation under the bundle lock validates/fsyncs the
  final directory entry, unlinks marker, removes wrapper, fsyncs changed parent
  directories, and then applies normal cheap reuse.
- Add a checked small production build+Ticket005 transport fixture. Specs with
  isolated HOME/environment and real subprocesses cover every path precedence/
  invalidity case, exact JSON/errors, install/path/cheap/full verify, removal of
  original transport, idempotent reuse across a different transport ID for the
  same bundle with requested/installed provenance distinguished and no part
  reads, corrupt receipt/final conflict, same-size payload/NOTICE tamper
  (cheap trust disclosure and full detection), live lock/timeout/process death,
  same/different-bundle concurrency, permissions/symlinks/rename races,
  failpoints before each sync/rename plus post-rename/pre-fsync and marker/
  wrapper cleanup, SIGKILL at staged checkpoints, and safe next-run
  reconciliation, including kill after marker unlink/before rmdir and removal
  of only the proven-empty markerless wrapper.
- Add one valid Ticket005 transport totaling at least 64 MiB (therefore one
  canonical final part), plus a lower-level bounded-chain iterator test over
  1000 small synthetic regular handles that is not called a valid transport.
  A Linux subprocess tracking allocator and `/proc/self/fd` regression must
  enforce <=16 MiB peak allocator delta and <=8 additional open FDs during the
  valid install and synthetic iterator, independent of aggregate bytes/handle
  count. Prove cheap reuse does not open parts, hash NOTICE/scores, or
  traverse ordinary payload. Retain
  `planning/artifacts/006-local-install-performance.md` with install/reuse/
  cheap/full-verify latency, allocations, peak RSS/fault caveat, bytes read and
  written, staging apparent-byte high-water, contention/timeout results, and
  transport-removal proof. Evidence is not a latency threshold.
- Update README, delivery/runtime-data architecture, ADR 0005, FAQ, and frontier
  to mark Linux explicit local installation as shipped. Network download,
  cache, embedded release lock/default, automatic first-start resolution,
  model assets, garbage collection/repair, other OSes, and HTTP remain future.
- Excluded: cache, file URL/HTTP/GitHub, retries/resume/proxy/TLS/signing,
  publication, automatic lookup discovery/install, current pointer/default
  bundle, removal/repair/GC/downgrade, model/reference/mask/executable assets,
  self-update, containers beyond offline prefetch/mount documentation, and HTTP.

## Success Checklist

- Local Ticket005 parts install exhaustively and atomically to the exact Linux
  layout under a per-bundle kernel lock; same bundle IDs deduplicate regardless
  of transport ID and different IDs do not serialize.
- Exact path semantics, machine output/error contract, lock timeout, receipt,
  no-follow filesystem boundary, failpoint/SIGKILL lifecycle, and conflict
  behavior are observable with real-process specs.
- Cheap reuse is explicitly receipt/immutability trust, avoids payload scans,
  and requires no transport; full verify detects same-size tampering.
- Scaled tests prove bounded heap/FD use and retained evidence characterizes
  resource/lock behavior without claiming network or other-platform support.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. Install locally before downloading; the later downloader gets one proven
   filesystem sink and introduces cache/network policy separately.
2. Key installed data and locks by canonical bundle ID; transport ID belongs to
   packaging provenance only.
3. Require bundle IDs and omit a current pointer until a real embedded release
   lock/default and update policy exist.
4. Scope atomicity and threat-model claims to Linux/XDG and use real dirfd,
   no-follow, flock, no-replace rename, fsync, subprocess, and SIGKILL tests.
5. Trust an immutable manager receipt for cheap reuse; reserve full hashing and
   payload traversal for `--full`, and fail rather than repair corrupt finals.
6. Omit cache entirely because local parts can stream directly; cache belongs
   with a later network/resume ticket.

## Dependencies

Ticket 005. This is a dependency-gated design draft, not implementation-ready.
After Ticket 005 ships, a fresh independent reviewer must reconcile every
transport type/error/API/fixture assumption before this ticket can become
`ready`.

## Notes

- Tests must never inspect/mutate the developer's real HOME/XDG tree.
- Local transport is untrusted input; the trusted boundary begins only after
  exhaustive verification and immutable publication under the trusted base.
- No external release or network action is authorized.

## Independent Ticket Review

Reviewer: `next_packet_review` (independent, read-only, packet review)

Result: approved only as a dependency-gated `proposed` design draft. Review
made installed directories and locks bundle-ID keyed, removed cache/network and
mutable-current scope, required explicit bundle IDs, selected a Linux/XDG
dirfd/flock/no-replace/fsync boundary, pinned path/CLI/receipt/mode/trust rules,
defined one-pass first install versus manifest-only cross-transport reuse, and
made staging/SIGKILL recovery exact. No material packet finding remains.

This is not implementation-ready. After Ticket 005 ships, a fresh independent
reviewer must reconcile every assumed transport type, error, API, receipt, and
fixture with the actual implementation before changing this status.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending
