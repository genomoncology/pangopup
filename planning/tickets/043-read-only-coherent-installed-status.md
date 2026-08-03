# 043 — Read-only coherent installed status

Status: complete

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

Developer: `ticket043_implementation` (independent implementation sub-agent)

- Replaced combined status's read-write/exclusive open-or-create operation with
  an existing-file, descriptor-relative read-only observation path using
  `O_RDONLY | O_NOFOLLOW | O_CLOEXEC` and a nonblocking shared `flock`.
  Installers retain the existing exclusive lock. Missing root, missing lock
  authority, acquired coherent guard, and exclusive-lock contention remain
  distinct outcomes.
- Added generic tests for two concurrent shared observers, exclusion of an
  installer, wrong-mode/non-regular/symlink lock rejection, absent-root
  noncreation, installed-SNV partial state without filesystem mutation,
  activated state without lock authority, prompt best-effort contention, and
  fixture-only compatible combined readiness through the ordinary runtime
  validation/status implementation.
- Extended final-image qualification after the shipped miniature SNV install.
  The literal network-disabled command uses the image's UID/GID 65532,
  read-only root filesystem, `/var/lib/pangopup:ro`, and writable cache volume;
  it asserts `partial`, SNV `ready`, runtime `missing`, and
  `installing: false`. No production asset or test-only CLI path is used.
- Updated `README.md`, `architecture/delivery.md`, `spec/local-assets.md`,
  `spec/container-image.md`, `scripts/qualify-container.sh`, and
  `planning/frontier.md`. The existing malformed-runtime spec fixture also now
  supplies the lock authority required by its stated test subject, and the
  container delivery check asserts the new final-image stage.
- Focused evidence: `cargo test -p pangopup-assets` passed 96 tests;
  `cargo clippy --locked --package pangopup-assets --all-targets -- -D warnings`
  passed; `make spec` passed 246 tests with 7 intentionally skipped; and
  `bash tests/container-delivery.sh` passed. A locally built stripped AMD64
  image passed `scripts/qualify-container.sh` with the new read-only named-
  volume proof (`architecture=amd64`, `image_size=55,566,875` bytes). The
  helper removed its temporary containers, volumes, and work directory.
- No scoring, asset format, path, response schema, installer, sync, model, or
  publication behavior changed. There was no scope-relevant deviation.

## Adversarial Code Review

Reviewer: `ticket043_code_review` (independent read-only sub-agent)

Accepted with no material findings. The reviewer verified the descriptor-
relative `O_RDONLY | O_NOFOLLOW | O_CLOEXEC` authority open, inherited trust
checks, shared-reader/exclusive-installer lifetime, missing-authority failure,
prompt best-effort contention, unchanged successful JSON, and the absence of
payload scanning, hashing, scoring, or model initialization. It also confirmed
that fixture-only runtime installation remains test-only, that the shipped CLI
drives the literal final-image `/var/lib/pangopup:ro` proof, and that the two
additional existing acceptance files are necessary rather than scope creep.

Independent review checks passed: `git diff --check`, assets clippy with
warnings denied, and all 96 `pangopup-assets` tests. The pre-existing Cargo
warning about build metadata in the pinned `zstd-sys` requirement is unrelated.

## External Effect Evidence

Coordinator: not applicable

This ticket performs no public or irreversible external effect.

## Coordinator Final Check

Coordinator: Codex

The complete final gate passed: `make lint`, `make test`, and `make spec` (246
passed, 7 intentionally skipped). This includes the full locked Rust workspace,
executable-delivery checks, production-release qualification harness, and the
offline mustmatch suite. The only warnings are the pre-existing allowed
duplicate-dependency report and `zstd-sys` build-metadata warning.

The first stale-claim scan found that `planning/frontier.md` still described
the bug as current and the ticket as awaiting review. The same developer
corrected only that paragraph, and the same code reviewer accepted the
remediation. A final scan across `README.md`, `architecture/delivery.md`, the
three affected specs, and `planning/frontier.md` found no remaining shipped-
versus-future contradiction. `git diff --check` passes; no external effect was
performed.
