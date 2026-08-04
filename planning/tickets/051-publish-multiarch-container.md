# 051 — Publish and qualify the multi-architecture container

Status: ready

## Why

Pangopup has a qualified, non-root Dockerfile for native Linux AMD64 and ARM64,
and Apple Silicon testing has proved the ARM64 image works under Docker Desktop.
Users still have to clone the repository and build it themselves. The next
small product outcome is one public GHCR image that Docker selects correctly on
either architecture and that preserves the already-qualified CLI, service,
asset-volume, and cache behavior.

## Scope

- Add one manually dispatched, exact-commit GitHub workflow that builds the
  existing Dockerfile natively on `ubuntu-24.04` and `ubuntu-24.04-arm`, pushes
  exactly one unattested leaf image per architecture by digest, qualifies both
  anonymously, and creates one multi-architecture manifest at
  `ghcr.io/genomoncology/pangopup` only after both native jobs pass.
- Publish version tags `0.2.0` and `v0.2.0` plus the ordinary moving `latest`
  tag. Treat the manifest digest, not any tag, as the immutable deployment
  identity. The reviewed workflow refuses an observed pre-existing version tag,
  serializes publication through one fixed concurrency group, rechecks tag
  absence immediately before manifest creation, and verifies final resolution.
  GHCR has no atomic create-if-absent tag operation, so an out-of-band writer
  remains an explicitly documented race rather than a false immutability claim.
- Keep assets outside the image. The image continues to run as numeric
  `65532:65532`, defaults to the foreground HTTP service, and uses the durable
  data and disposable cache mounts already documented.
- Add a checked, credential-free publication/qualification record under
  `planning/artifacts/051-public-container.md`. It must authenticate that
  `origin/main`, the dispatch ref, the executing workflow revision, and the
  checked-out source all equal the exact reviewed commit; authenticate green
  CI/container gates; dispatch once; capture the manifest and child digests;
  require inherited public visibility; and verify anonymous pulls by digest and
  version tag. There is no private-package visibility fallback: inability to
  pull either digest anonymously is a hard stop before version tags exist.
- Extend the miniature container helper to authenticate an expected registry
  child digest from the pulled image while retaining its existing metadata,
  CLI, lookup, model, cache, and 75 MiB checks. Invoke it after logging out of
  GHCR on both native runners before manifest/tag creation. Do not enable the
  test-only service fixture profile, mix a miniature SNV bundle with the
  production runtime, or provision the 15 GB production SNV bundle on ordinary
  hosted runners. HTTP correctness remains proved by the normal Rust/service
  specs and retained full-volume Apple Silicon acceptance; this ticket does not
  claim a fresh full HTTP run on both hosted architectures.
- Update `README.md`, `architecture/README.md`, `architecture/delivery.md`,
  `architecture/service.md`, `planning/frontier.md`, `spec/container-image.md`,
  `spec/readme-first-use.md`, and `tests/container-delivery.sh` so the first-use
  path uses `docker pull ghcr.io/genomoncology/pangopup:0.2.0`, explains digest
  pinning and updates, and retains the local-build fallback and Apple CPU-only
  boundary.
- Preserve the existing executable release, runtime/SNV releases, model,
  lookup index, Dockerfile behavior, scoring results, cache schema, and public
  API.
- Do not add Compose, Kubernetes, systemd, an in-process supervisor,
  orchestration-specific health policy, MPS/Metal, a custom ONNX Runtime,
  embedded assets, or automatic publication on every push.

## Success Checklist

- Static tests prove the publication workflow is manual, requires a lowercase
  40-character commit exactly equal to `GITHUB_SHA`, `github.workflow_sha`, the
  checkout, and current `main`; uses a fixed concurrency group; gives only leaf
  build and manifest jobs package-write permission; builds both architectures
  natively; pins every action by full commit; disables provenance/SBOM for this
  deferred-attestation ticket; exchanges uniquely named artifacts containing
  exactly one valid SHA-256 digest; registry-inspects distinct AMD64/ARM64 leaf
  manifests and exact labels; and checks version-tag absence both before builds
  and immediately before assembly.
- The ordinary push/PR container workflow remains read-only and continues to
  pass native AMD64 and ARM64 miniature smoke tests without publication power.
- Before the external effect, the exact publication-ready commit passes `make
  lint`, `make test`, `make spec`, remote `ci/gate`, and the native container
  smoke matrix.
- The public GHCR package has one OCI index with exactly two entries and no
  attestation/unknown-platform children. Its `linux/amd64` and `linux/arm64`
  leaf digests are exactly the reviewed native outputs. Tags `0.2.0` and
  `v0.2.0` resolve to that index; `latest` resolves to it at publication time.
  The workflow observed both version tags absent twice and records the
  unavoidable out-of-band tag race.
- Anonymous pulls of both leaf digests succeed before tag creation; pulls by
  exact index digest and by `0.2.0` succeed afterward. Native qualification on
  both architectures proves the held pulled child digest,
  labels, non-root/container layout, and miniature CLI/model/cache behavior.
  The image pull contains and downloads no scoring assets. Existing service
  tests and retained full-volume Apple qualification remain linked as the HTTP
  evidence; full two-architecture production-asset HTTP qualification would
  require separate large/self-hosted runner infrastructure.
- README Docker quick start is copy/pasteable for sync, status, lookup, service,
  `curl`, update, digest pinning, and selective image/volume removal.
- The public evidence records only non-sensitive commit/run/package/digest/tag
  facts. It contains no token, authorization header, signed URL, environment
  dump, private filesystem path, or registry credential material.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Registry:** GitHub Release archive versus GHCR. Use GHCR because Docker can
   select the native child from one OCI index and the public repository already
   uses GitHub for source and immutable data/executable releases.
2. **Build strategy:** emulation versus native runners. Use the existing native
   AMD64 and ARM64 GitHub runners. This avoids QEMU inference ambiguity and
   extends the qualification path already proven by the read-only workflow.
3. **Publication trigger:** every push/tag versus reviewed manual dispatch. Use
   an exact-commit manual workflow whose own workflow revision must be that same
   commit. Container publication is an irreversible public effect and must not
   happen on ordinary CI or merely because a tag is pushed.
4. **Tags:** only mutable `latest` versus version plus digest. Publish `0.2.0`,
   `v0.2.0`, and `latest`, document version tags for humans and the OCI digest
   for immutable deployment. Serialize the reviewed workflow and fail closed on
   an observed version-tag collision, while stating that GHCR cannot provide an
   atomic no-overwrite guarantee against an unrelated writer.
5. **Image contents:** bundle the 15 GB installed data versus thin runtime.
   Retain the thin image selected by ADR 0026. Users synchronize the independently
   versioned assets into named volumes; image updates do not redownload or
   republish them.
6. **Supply-chain scope:** add every possible attestation/signing system now or
   first publish a minimal inspectable image. Require exact commits, pinned
   actions, OCI source/revision/version/license labels, held digests, native
   qualification, and public digest verification. Defer signing beyond the
   authenticated GitHub publication record unless a later threat model justifies the
   extra keyless-signing surface.

## Dependencies

- Ticket 050 complete: immutable public executable `v0.2.0`, compact README,
  and qualified public installer.
- Existing native AMD64/ARM64 Dockerfile and read-only container qualification.

## Notes

- Only the coordinator may dispatch the publish workflow or create public
  tags/manifests. Development and review
  stop at `publication-ready`. The workflow-created package must inherit public
  visibility from this public repository; the coordinator does not rely on an
  unproved package-admin token to repair a private package.
- Package publication must target `genomoncology/pangopup`; avoid similarly
  named personal namespaces.
- A public failure after a manifest is visible is recorded honestly and stops
  work. Do not delete or mutate an observed public version tag to hide a failed
  qualification.
- The harmless Apple Docker ONNX CPU-vendor warning is accepted. The image is
  native Linux ARM64 CPU inference; it does not use macOS MPS/Metal.

## Coordinator Authorship

Coordinator: Codex. Drafted from the completed Ticket 050 release, ADR 0026,
the current native container workflow, the successful Apple Silicon evidence,
and the rolling frontier.

## Independent Ticket Review

Reviewer: independent design reviewer `ticket051_design_review` — ACCEPT.

The first review rejected an infeasible private-to-public fallback, overstated
tag immutability, insufficient workflow/source binding, ambiguous nested
manifest construction, and qualification that did not authenticate the held
registry child. The revision requires inherited public visibility before tags,
states the unavoidable GHCR tag race, binds the executing workflow and source
to the same exact current-main commit, publishes exactly two unattested leaf
digests, and extends the shared helper for held-child authentication.

Re-review then established that a production HTTP service cannot run from the
miniature fixture or runtime-only transport: it correctly requires a matching
15 GB SNV installation. The final ticket keeps two-architecture hosted-runner
qualification to exact pulled digest/layout/miniature lookup/model/cache,
retains HTTP proof in service specs and completed full-volume Apple evidence,
and explicitly leaves two-architecture full-asset HTTP to separate large-runner
infrastructure. The reviewer accepted the final bounded design.

## Implementation Evidence

Developer: pending.

## Adversarial Code Review

Reviewer: pending.

## External Effect Evidence

Coordinator: pending. This ticket includes reviewed public GHCR publication and
must stop at `publication-ready` until the exact commit is pushed and its
required remote gates are green.

## Coordinator Final Check

Coordinator: pending.
