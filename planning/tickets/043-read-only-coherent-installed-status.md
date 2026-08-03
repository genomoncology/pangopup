# 043 — Read-only coherent installed status

Status: ready

## Why

Apple Silicon qualification of the shipped Docker image passed native ARM64
build, asset synchronization and resume, offline reuse, SNV lookup, model
fallback, persistent SQLite reuse, HTTP scoring, and container restart. It also
found one deterministic product defect in the documented safe-use path:

```text
docker run --rm --network none --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  -v pangopup-mac-data:/var/lib/pangopup:ro \
  -v pangopup-mac-cache:/var/cache/pangopup \
  pangopup:mac-arm64 status
```

The command exits 1 with `ASSET_STATE_INVALID` and `install lock cannot be
opened without following links`. Removing only `:ro` makes the same installed
profile report `ready`; lookup and the foreground service already work with
the data mount read-only.

The failure is not a Docker or Apple-specific scoring problem. Combined status
currently calls `acquire_existing_install_lock`, which opens or creates
`.install.lock` with read-write access and resets its mode. Observation should
not mutate the installed asset store. This ticket makes status genuinely
read-only while preserving a coherent view across SNV and runtime state.

## Scope

- Give combined local status a separate read-only observation guard. Open the
  existing `.install.lock` through the held root directory descriptor with
  `O_RDONLY`, `O_NOFOLLOW`, and `O_CLOEXEC`; validate the same ownership,
  same-filesystem, regular-file, and mode invariants as the existing safe
  metadata readers.
- Hold a nonblocking shared `flock` while observing both SNV and runtime state.
  Installation retains its existing nonblocking exclusive lock. Multiple
  status readers may coexist, while an active installer makes status return
  promptly with the established `installing: true` observation. Component
  observations under contention remain best-effort and are not cross-validated;
  coherence is promised only while status successfully holds the shared guard.
- Do not create `.install.lock`, create the data root, change permissions, or
  write any installed-state path during status. A missing root remains
  `missing` and remains absent.
- Treat activated installed state without the required existing lock authority
  as invalid. Status must not silently create or repair the authority file.
  Preserve rejection of symlinks, non-regular files, unexpected ownership or
  filesystem, and mode.
- Preserve the current public CLI, JSON fields, exit behavior for valid
  `missing`/`partial`/`ready`/`installing` observations, paths, XDG defaults,
  asset formats, and installer behavior.
- Add inside-out unit/integration tests for missing, partial, ready, concurrent,
  and hostile lock states. Crate tests may use the already-established
  fixture-only runtime-profile installer to prove a miniature combined `ready`
  state through the ordinary runtime validation/status code; do not expose or
  add that fixture path to the shipped CLI. Extend final-image qualification
  with a literal network-disabled, read-only-root Docker invocation whose
  checked miniature SNV installation reports `partial` with its data named
  volume mounted `:ro`. Use the shipped CLI and existing production admission
  paths in the final image; do not add a test-only asset-profile bypass or
  download production assets in normal gates.
- Keep status bounded to metadata and pointer observation. It must not scan or
  hash score/reference payloads or initialize the model.
- Update `README.md`, `architecture/delivery.md`, `spec/local-assets.md`,
  `spec/container-image.md`, `scripts/qualify-container.sh`, and
  `planning/frontier.md` so the read-only observation contract and its
  executable proof agree. Existing ADRs 0021 and 0026 own the lock and
  container-mount decisions; do not add a redundant ADR.
- Exclude focused subcommand help, sync retry/progress, ONNX warning cleanup,
  Apple CPU tuning, systemd, executable or container publication, registry
  manifests, Compose, asset-format changes, and scoring changes.

## Success Checklist

- `pangopup status --data-dir <absent>` returns the established missing JSON
  and leaves `<absent>` nonexistent.
- Status observes a checked miniature SNV-only installation as `partial` and,
  in crate tests through the existing fixture-only installer, a checked
  compatible miniature combined installation as `ready` when the data tree is
  not writable. Before/after filesystem evidence proves that status creates or
  changes no entry.
- Two status readers can hold the observation guard concurrently. An installer
  holding the exclusive lock causes status to return promptly with
  `installing: true`; it does not wait. Component observations in that
  contention response are explicitly best-effort and not cross-validated.
- Missing lock authority for activated state and hostile `.install.lock`
  shapes fail closed with the existing typed asset errors. The status path does
  not repair them.
- The final stripped image passes a literal `docker run --network none
  --read-only` partial-status check as UID/GID 65532 after installing the
  checked miniature SNV through the shipped CLI, with its data named volume
  mounted `/var/lib/pangopup:ro` and its cache volume writable. This is
  exercised by the bounded container qualification helper, not inferred from
  a host library test. Full combined-ready validation remains in crate tests;
  the final-image proof does not weaken production runtime-profile admission.
- Existing status JSON/spec examples remain byte-compatible except where a new
  failure-path example is intentionally added. SNV lookup, model fallback,
  sync, and installation behavior remain unchanged.
- Normal gates remain offline and asset-independent. `make lint`, `make test`,
  and `make spec` pass.

## Decisions

### Fix observation rather than weaken the documented mount

Options were to remove `:ro` from the Docker example, document a writable
exception for status, or make status read-only. Lookup and service operation
already work safely with `:ro`, and an observational command has no reason to
write installed assets. Preserve the safer mount contract and fix status.

### Shared observation lock versus an unlocked snapshot

Reading SNV and runtime pointers without a guard can combine states from two
different installation moments. Reusing the exclusive installer lock would
still require write access and unnecessarily serialize status readers. Use a
nonblocking shared lock for status and retain the existing exclusive lock for
installation. Successfully taking the shared guard gives a coherent snapshot
without mutation. If an installer holds the exclusive lock, preserve the
current nonwaiting contract: return `installing: true` with best-effort
component observations and no coherence claim.

### Existing lock authority versus implicit repair

Creating `.install.lock` during status makes observation alter the store and
hides incomplete or tampered installations. A wholly missing store needs no
lock and reports missing. Once activated state exists, its established lock
file is required; absence is invalid and only an explicit install/sync path may
repair or replace state.

### Stable public response versus a new status protocol

The failure is below the CLI schema. Adding JSON fields or a new mode would
make clients absorb an implementation correction. Preserve the existing
status contract and exit semantics; only the erroneous read-only failure
changes to the status already returned on a writable mount.

### Final-image proof versus library-only confidence

The defect was exposed by the actual distroless image, non-root user, read-only
root filesystem, and read-only named volume. Unit tests are necessary for lock
semantics but cannot prove that deployment composition. Keep a literal
final-image regression in container qualification as the acceptance authority.

## Dependencies

- Ticket 042's shipped minimal qualified Docker image.
- ADR 0021's held-descriptor local asset-store trust boundary.
- ADR 0026's read-only non-root container posture and explicit named volumes.

## Notes

- The external Mac report is evidence, not a repository input. Do not copy its
  raw logs, absolute user paths, SQLite snapshots, or Docker volumes into the
  repository.
- The exact failure was reproduced twice. The same data volume returned
  `ready` when only its `:ro` suffix was removed, while lookup and service use
  succeeded with that suffix present.
- `probe_install_lock` already demonstrates safe read-only opening and
  nonblocking probing, but it does not hold a guard across both component
  reads. Reuse its trust checks rather than introducing a second permissive
  path.
- The shipped runtime-profile installer intentionally admits only trusted
  production identities. The existing fixture-only installer is valid for
  crate tests, not for the final image or public CLI. The final-image regression
  therefore uses the real miniature SNV transport and expects `partial`.
- Container qualification must clean only resources it creates and must not
  fetch, inspect, or rebuild production assets.
- The gate is `make lint`, `make test`, and `make spec`; there is no `make
  check`.

## Coordinator Authorship

Coordinator: Codex

The coordinator authored this ticket from the completed Ticket 042 outcome,
the rolling frontier, and the independent Apple Silicon validation. It does
not implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: `ticket043_design_review` (independent read-only sub-agent)

The first review rejected three contradictions. A combined-ready miniature
profile cannot be installed through the production CLI, so the ticket now
keeps that proof in crate tests through the already-established fixture-only
installer and uses a real shipped-CLI miniature SNV `partial` state for the
final-image regression. A nonblocking status cannot promise a coherent
snapshot while an installer owns the exclusive lock, so coherence now applies
only under the acquired shared guard and contention explicitly retains
best-effort components with `installing: true`. Finally, the existing trust
path does not reject additional hard links, so that inaccurate preservation
claim was removed rather than expanding installer security scope.

Re-review: accepted. The reviewer confirmed that the corrected shared/exclusive
lock split, missing-authority behavior, trust checks, miniature proof boundary,
metadata-only bound, named documentation, and exclusions are internally
consistent and feasible. There are no remaining material findings.

## Implementation Evidence

Developer: pending

Record focused tests, measurements, generated artifact identities, and any
scope-relevant deviation, including named documentation changes, then set
status to `review`. The developer cannot be the author or either reviewer and
does not commit or push.

## Adversarial Code Review

Reviewer: pending

Record diff/test findings and their disposition before completion. The
reviewer is read-only and cannot be the author, ticket reviewer, or developer.
Material fixes return to the same developer and then this reviewer. Review
includes every named documentation file and a check that shipped and future
behavior are not confused. The ticket may become `complete` and enter final
gates only after the reviewer records approval.

## External Effect Evidence

Coordinator: not applicable

This ticket performs no public or irreversible external effect.

## Coordinator Final Check

Coordinator: pending

Record final `make lint`, `make test`, and `make spec` results plus a
documentation stale-claim scan. The coordinator authors and remediates the
ticket, orchestrates the independent stages, records evidence, and commits and
pushes approved work; it does not implement product code or review its own
ticket. A material final-gate or documentation finding returns to the same
developer and code reviewer; a scope defect returns to the coordinator and
same ticket reviewer.
