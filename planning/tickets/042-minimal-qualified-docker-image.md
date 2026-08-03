# 042 — Minimal qualified Docker image

Status: complete

## Why

Pangopup now has a foreground HTTP service and pinned downloadable runtime
assets, but it has no container image. The next smallest useful deployment
primitive is one thin image built from a Dockerfile. Compose orchestration and
public registry publication would add separate lifecycle and external-effect
decisions, so they do not belong in this ticket.

The image must not contain the roughly 15 GB installed SNV index or the
model-side runtime assets. The same executable must instead support an explicit
one-shot `sync` container using persistent storage and a later foreground
`serve` container mounting that storage. This preserves the existing rule that
only `pangopup sync` performs network provisioning.

## Scope

- Add one allowlisted `.dockerignore` and one multi-stage `Dockerfile` that
  build the existing `pangopup` executable for native Linux AMD64 and ARM64.
- Use the exact multi-platform builder authority
  `rust:1.93.1-trixie@sha256:ecbe59a8408895edd02d9ef422504b8501dd9fa1526de27a45b73406d734d659`
  and runtime authority
  `gcr.io/distroless/cc-debian13:nonroot@sha256:d97bc0a941b8d4be647dc0ee75b264ddbb772f1ac5ba690a4309c00723b23775`.
  The builder must use `cargo build --locked --release --package
  pangopup-cli`, strip the copied executable, and leave no build tools in the
  final stage.
- The final image contains only the runtime base, `/usr/local/bin/pangopup`,
  `LICENSE`, `NOTICE`, and empty writable mountpoints. It runs as numeric
  UID/GID `65532:65532`, exposes 8080, uses exec-form `pangopup` as entrypoint,
  defaults to `serve --listen 0.0.0.0:8080`, and declares `SIGTERM` as its stop
  signal.
- Set `PANGOPUP_DATA_DIR=/var/lib/pangopup`,
  `PANGOPUP_CACHE_DIR=/var/cache/pangopup`, and
  `PANGOPUP_MODEL_CACHE=/var/cache/pangopup/model-results.sqlite3`. Ensure a
  fresh Docker named volume mounted at either directory is writable by the
  image user. Do not declare Dockerfile `VOLUME` entries, because an omitted
  explicit mount must not silently create an anonymous multi-gigabyte volume.
- Add OCI title, source, revision, version, description, and
  `GPL-3.0-only` license labels. Revision and version are explicit build
  arguments; local defaults may say `unknown`, while qualification supplies
  the exact commit and workspace version.
- Add a host-side, bounded container qualification helper and a read-only
  GitHub workflow that build and exercise the final image natively on
  `ubuntu-24.04` and `ubuntu-24.04-arm`. It may produce workflow logs but must
  not upload or publish an image, package, manifest, attestation, or release.
- Add a small checked public-output oracle for the 14 scored cases in
  `pangolin-compat-v1`. A normal offline Rust test must derive the expected
  records from the frozen corpus and prove that this oracle has not drifted.
  The production container qualification must pass all 14 requests in one
  ordered `pangopup lookup --model-only --format jsonl` call to the executable
  inside the final image, using mounted authenticated model, reference, and
  mask assets, and compare the complete public JSONL output to that oracle.
  This final-image result—not a host-side library test—is the acceptance
  authority. The existing ignored production library test may be generalized
  to explicit paths and retained only as a diagnostic.
- Correct the root `NOTICE`, which still says Pangopup does not distribute a
  Pangolin model even though `runtime-grch38-v1` now publishes the converted
  model and GPL preferred source.
- Document direct `docker build`, named-volume `sync`, `status`, `lookup`, and
  foreground `serve` use in `README.md`. Update
  `architecture/delivery.md`, `architecture/service.md`,
  `architecture/README.md`, add
  `architecture/decisions/0026-minimal-container-image.md`, and update
  `planning/frontier.md`, `planning/faq.md`, and `AGENTS.md` so shipped and
  future claims agree.
- Exclude Docker Compose, an image-native healthcheck command, restart policy,
  GHCR or other registry publication, multi-platform manifest publication,
  image SBOM/attestation/signing, Kubernetes, systemd, TLS/authentication,
  accelerator support, and changes to score or asset formats.

## Success Checklist

- `docker build` from the repository produces a native final image whose
  config has user `65532:65532`, the intended entrypoint/default command,
  exposed port, stop signal, environment, and OCI labels.
- Host inspection of a created/exported container proves the final image has
  no shell or package manager and contains no `.pgi`, `.pgr`, `.pgm`, `.onnx`,
  raw source dataset, checkpoint, Python environment, Cargo target, or Git
  metadata. `docker image inspect --format '{{.Size}}'` reports at most
  78,643,200 bytes (75 MiB) on both supported architectures; this is the local
  uncompressed content size, not a compressed registry-transfer claim.
- The final image runs `--version`, exact missing `status`, an installed
  miniature SNV lookup, and miniature ONNX model inference as non-root with
  `--read-only`, a bounded `/tmp` tmpfs, and explicit named data/cache volumes.
  The ordinary smoke must install the miniature SNV through its real transport
  path and run the explicit miniature model/reference/mask tuple; it must not
  add a test-profile bypass to production runtime admission. It must prove
  SQLite cache creation and a cache hit by copying the database from the named
  cache volume before and after the second call and showing that the second
  call leaves it byte-identical. This proves that fresh named volumes at both
  `/var/lib/pangopup` and `/var/cache/pangopup` are writable by UID/GID 65532
  rather than merely inferring ownership from image metadata. Ticket 042 does
  not re-prove the combined production runtime-profile installer, because that
  installer intentionally requires the already-installed 15 GB production SNV
  bundle and this ticket forbids downloading it. The already-shipped installer
  qualification remains its authority.
- The final image's default command fails before listening with the existing
  stable missing-assets error when the data volume lacks a complete profile;
  overriding the command with `sync`, `status`, `lookup`, and explicit `serve`
  arguments reaches the existing CLI grammar without a wrapper shell.
- Native AMD64 and ARM64 jobs run the checked miniature SNV/model paths.
  Before either architecture is described as production model compatible, a
  manually dispatched exact-commit matrix qualification downloads only the
  existing 691,874,664-byte `runtime-grch38-v1` transport, verifies every
  declared size/SHA-256, unpacks it with `pangopup-build`, mounts the explicit
  authenticated model/reference/mask tuple read-only, and runs the entire
  ordered 14-case batch at `sequential:1/1` through `pangopup lookup
  --model-only` inside that architecture's final stripped distroless image. It
  must not download or reconstruct the 15 GB SNV artifact or claim to re-test
  the combined installed-profile path.
- Both final-image results preserve the established public hundredth scores,
  positions, record and request order, warnings, errors, and model/reference/
  mask identities. A mismatch blocks that architecture's support claim; the
  workflow must not substitute a host-built test executable, weaken
  compatibility, or silently fall back to emulation. The existing Rust
  production qualification may additionally diagnose a failure but cannot
  satisfy this requirement.
- Container qualification is separate from normal gates. `make lint`, `make
  test`, and `make spec` remain offline, asset-independent, and pass. The
  qualification helper is bounded, cleans only paths/containers/volumes it
  created, and never deletes a caller-owned or production asset volume.
- User documentation shows direct commands equivalent to:

  ```text
  docker volume create pangopup-data
  docker volume create pangopup-cache
  docker run --rm --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m -v pangopup-data:/var/lib/pangopup -v pangopup-cache:/var/cache/pangopup <IMAGE> sync
  docker run --rm --read-only --tmpfs /tmp:rw,noexec,nosuid,size=64m -p 127.0.0.1:8080:8080 -v pangopup-data:/var/lib/pangopup:ro -v pangopup-cache:/var/cache/pangopup <IMAGE>
  ```

  It states that `docker rm` and ordinary image upgrades preserve named
  volumes, while explicit volume deletion removes installed assets and cached
  model results.

## Decisions

### Thin image versus embedded runtime assets

The installed SNV member alone is 15,033,158,255 bytes. Baking it into an image
would duplicate it on image upgrades, make image distribution dominate the
software release, and defeat the existing pinned sync/install boundary.
Options were a thin image with explicit mounts, a roughly 16 GB asset-bearing
image, or an entrypoint that implicitly downloads before every serve. Use the
thin image. `sync` remains an explicit command and `serve` remains network-free.

### Dockerfile primitive versus orchestration

The image must be independently usable before a higher-level launcher is
introduced. Options were Dockerfile plus Compose in one ticket, Dockerfile
alone, or Compose around a pre-existing host binary. Ship the Dockerfile alone
with literal `docker run` examples. Compose and healthcheck orchestration are a
later outcome built on this qualified primitive.

### Runtime base and health probing

The selected static ONNX Runtime objects require C23 glibc symbols absent from
Debian 12. A local probe showed the Debian 12 Rust image fail at link with
missing `__isoc23_strto*`, while the pinned Debian 13 builder produced a
27,989,488-byte stripped executable that ran in the pinned Debian 13
distroless `cc` image. Options were Ubuntu 24.04 plus curl, Debian 13 slim, or
Debian 13 distroless. Use distroless to retain the existing no-shell,
no-package-manager direction. Do not add a new CLI healthcheck merely to fill a
Dockerfile field; the existing `/livez` and `/readyz` endpoints remain the
service contract and Docker-native probing belongs with later orchestration.

### Architecture qualification

The ONNX Runtime dependency supplies checksum-pinned CPU archives for both
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, but availability is
not correctness. Options were AMD64 only, cross-built ARM64, or native two-arch
qualification. Build and test on native runners. ARM64 uses Linux CPU ONNX
inference; it does not claim macOS MPS or any accelerator.

### Publication boundary

Building and qualifying an image is reversible local/CI work; publishing a
registry package is a new public external effect with tag, provenance, SBOM,
and rollback decisions. Options were publish immediately or qualify first.
Qualify first. This ticket grants no `packages: write` permission and performs
no registry login or push.

## Dependencies

- Ticket 041's shipped foreground HTTP service.
- The immutable public `snv-grch38-v1` and `runtime-grch38-v1` assets and the
  shipped pinned `pangopup sync` command.

## Notes

- Work from the current repository. Preserve unrelated changes and never copy
  files from `/home/ian/workspace/data`, `$HOME`, `.git`, `target`, ignored
  Python environments, or planning evidence into the build context.
- The exact gate is `make lint`, `make test`, and `make spec`; there is no
  `make check`.
- Normal gates and pull-request container smoke must use checked miniature
  fixtures and must not fetch production assets. Only the explicit manual
  native production-model qualification may fetch the published runtime-only
  transport. The ordinary AMD64 first-install proof is explicitly miniature,
  not a production-asset fetch.
- Run Docker builds natively in each matrix job. Do not install QEMU or report
  an emulated result as platform qualification.
- The public repository contains no secrets. Workflow permissions remain
  `contents: read`; do not add credentials, registry login, `packages: write`,
  unpinned actions, or local absolute paths.
- Final image inspection and production qualification evidence are retained in
  a concise `planning/artifacts/042-minimal-container-image.md`; generated
  image archives, downloaded runtime members, and local Docker volumes are not
  committed.
- Evidence in this ticket is illustrative; do not commit generated build or
  download outputs.

## Coordinator Authorship

Coordinator: Codex

The coordinator authored this ticket from the shipped Ticket 041 service and
the Dockerfile-first product decision. It does not implement product code or
approve its own ticket.

## Independent Ticket Review

Reviewer: Popper the 2nd

The first review rejected a host-side 14-case test plus a one-case final-image
probe: that could miss a build/runtime difference in the stripped distroless
binary, and AMD64 also needed qualification of this exact image rather than a
previous Ubuntu-built executable. The ticket now requires the complete ordered
14-case public CLI result through each native final image. It also makes the
75 MiB measurement, two-volume write proof, and miniature versus production
fetch boundary explicit.

Re-review: approved. The reviewer confirmed that the final-image authority now
covers both native architectures, that host-binary substitution and emulation
are forbidden, and that the image-size and writable-volume proofs are exact.

Implementation clarification: the shipped runtime installer correctly rejects
miniature test profiles. Requiring a miniature runtime installation would add
a production bypass solely for container testing. The ordinary smoke therefore
installs the miniature SNV, mounts the explicit miniature model tuple, and
proves SQLite creation plus byte-stable reuse. The production runtime installer
also requires the bound production SNV, so the 691 MB manual matrix cannot
exercise installation without violating the ticket's no-15-GB-download rule;
it authenticates/unpacks and explicitly mounts the production tuple instead.
The previously shipped installer qualification remains authoritative.
Re-review: approved. The reviewer confirmed this is the feasible boundary
without downloading the 15 GB production SNV bundle or weakening runtime
profile admission.

## Implementation Evidence

Developer: Avicenna the 2nd

Implemented the pinned multi-stage distroless Dockerfile, allowlisted context,
numeric non-root runtime, explicit persistent paths, OCI configuration, native
two-runner read-only workflow, bounded miniature and runtime-only production
qualification helpers, corpus-derived 14-case public oracle, notice correction,
ADR, user documentation, and planning updates. The immutable SNV bundle notice
was split from the corrected root product notice without changing its bytes or
published identity.

Focused checks passed: the corpus/oracle Rust test, static container delivery
test, shell syntax checks, native AMD64 Docker build, and the complete bounded
final-image miniature qualification. Docker reported 55,564,827 bytes and the
expected `linux/amd64`, `65532:65532`, entrypoint, command, stop signal,
environment, port, and labels. The helper proved fresh named-volume writes via
real SNV transport installation and SQLite model-cache creation/reuse under a
read-only root. No image, workflow artifact, registry object, or release was
published. Native ARM64 and the 691,874,664-byte runtime-only 14-case production
run remain the deliberately manual read-only workflow evidence.

The complete offline gate passed: `make lint`, `make test`, and `make spec`
(246 passed, 7 skipped). The production helper authenticates and unpacks the
runtime-only transport, makes the decoded files read-only and traversable by
the image user, and explicitly mounts the model/reference/mask tuple for the
14-case model-only run. Splitting the immutable SNV notice from the root
product notice regenerated only the checked miniature's builder provenance and
derived identity; its payload, source, request, reference, and notice bytes are
unchanged, and historical and production identities remain pinned.

Durable detail: `planning/artifacts/042-minimal-container-image.md`.

## Adversarial Code Review

Reviewer: Ampere the 3rd

The first code review rejected four points: a reversed non-zero loss sign in
the new oracle, unreadable permissions after private production unpack, a
comparator that authenticated provenance only on the first result, and the
ordinary smoke's failure to prove SQLite reuse. It also identified the literal
miniature-runtime-install requirement as incompatible with the intentional
production profile trust boundary.

Remediation preserves the corpus's signed `-0.08` loss with an explicit
sentinel, makes only the authenticated decoded runtime tree read/traverse-only,
compares exact provenance on all 14 ordered results, and copies the SQLite file
before and after the second call to prove byte-stable reuse. Production
qualification now mounts the authenticated tuple explicitly, mounts its cache
volume at the image-prepared `/var/cache/pangopup`, and makes no runtime-install
claim. Focused oracle/static/shell/native-container checks and
the complete `make lint`, `make test`, and `make spec` gate pass. Re-review is
accepted after the remediation described below.

The remaining cache-mount finding is also remediated: production qualification
uses the image-prepared `/var/cache/pangopup` volume and its configured default
SQLite path, with a static regression check rejecting the former root-owned
`/cache` mount. The focused shell/static contract, `make lint`, and `make spec`
pass after this change.

The first pushed native smoke then failed at a shared early silent assertion on
both architectures. The helper now emits actionable stage/check/expected/
observed diagnostics for image metadata, and validates exposed ports through
canonical JSON rather than implementation-specific empty-value rendering. A
fresh no-cache AMD64 build passed the complete local qualification. Read-only
workflow run `30827959840` then passed the complete final-image smoke natively
on both AMD64 and ARM64; its production matrix was correctly skipped and it
published nothing.

Independent re-review accepted the canonical whole-object port check and
actionable diagnostics: all metadata constraints remain exact, cleanup remains
active, and no check was weakened.

Final re-review: accepted. The reviewer independently confirmed the signed
loss sentinel, decoded runtime permissions, all-result provenance comparison,
byte-stable SQLite reuse, correct non-root cache mount, scope, and
documentation. No material correctness or security findings remain.

## External Effect Evidence

Coordinator: not applicable — this ticket must not publish a container image
or mutate a registry.

## Coordinator Final Check

Coordinator: Codex

Reviewed the final worktree and reran `make lint`, `make test`, and `make spec`.
All passed; mustmatch reported 246 passed and 7 intentionally skipped. The
native AMD64 final-image qualification remains green at 55,564,827 bytes, and
workflow run `30827959840` passed the native AMD64 and ARM64 smoke jobs. The
manual native AMD64/ARM64 production-runtime matrix remains deliberately
dispatch-only and performs no publication. Ticket 042 is complete.
