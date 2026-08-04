# 051 — Publish and qualify the multi-architecture container

Status: publication-ready

## Why

Pangopup has a qualified, non-root Dockerfile for native Linux AMD64 and ARM64,
and Apple Silicon testing has proved the ARM64 image works under Docker Desktop.
Users still have to clone the repository and build it themselves. The next
small product outcome is one public GHCR image that Docker selects correctly on
either architecture and that preserves the already-qualified CLI, service,
asset-volume, and cache behavior.

## Scope

- Add one manually dispatched, exact-commit GitHub workflow with two explicit
  modes. `stage` builds the existing Dockerfile natively on `ubuntu-24.04` and
  `ubuntu-24.04-arm`, pushes exactly one unattested private leaf image per
  architecture by digest, qualifies both with the repository `GITHUB_TOKEN`,
  uploads one canonical digest artifact per native job, and creates one
  canonical combined two-leaf receipt only after both qualifications pass,
  without creating user-facing tags. `finalize` accepts the exact stage run ID,
  authenticates and downloads that run's combined receipt through the Actions
  API, derives the held digests from it, anonymously requalifies them on their
  native runners after the package is public, then creates the two-platform
  manifest and tags.
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
  CI/container gates; capture the staged child digests; and stop at a named
  visibility checkpoint. GitHub creates a new GHCR package as private and
  exposes no supported REST or GraphQL visibility mutation. The coordinator
  must give the organization owner the exact package-settings URL and wait for
  that one manual, irreversible public-visibility confirmation. Only then may
  the coordinator verify anonymous leaf access and dispatch `finalize` with the
  exact held stage run ID. Finalization verifies anonymous pulls by digest and
  version tag. A private or inaccessible package is a hard stop before tags.
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

- Static tests prove both workflow modes are manual, require a lowercase
  40-character commit exactly equal to `GITHUB_SHA`, `github.workflow_sha`, the
  checkout, and current `main`; uses a fixed concurrency group; gives only leaf
  stage-build and finalize-manifest jobs package-write permission; gives native
  anonymous-finalize qualification jobs no package permission; builds both
  architectures natively; pins every action by full commit; disables
  provenance/SBOM for this deferred-attestation ticket; exchanges uniquely
  named artifacts containing exactly one valid SHA-256 digest; requires the
  stage aggregation job to emit exactly one canonical two-leaf receipt only
  after both native qualifications pass; requires `finalize` to authenticate
  through the Actions API that the supplied stage run ID is a successful run of
  this exact workflow in `stage` mode at the same commit/workflow SHA;
  downloads exactly that retained receipt with job-scoped `actions: read`;
  rejects missing, expired, duplicate, noncanonical, or mismatched artifacts
  without rebuilding or selecting a latest run; derives finalization digests
  only from that receipt;
  registry-inspects distinct AMD64/ARM64 leaves and exact labels; and checks
  version-tag absence both before staging and immediately before assembly.
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
   a two-mode exact-commit manual workflow whose own workflow revision must be
   that same commit. Staging creates only private digest-addressed leaves;
   finalization is a separate coordinator dispatch after the required manual
   visibility checkpoint. Container publication must not happen on ordinary CI
   or merely because a tag is pushed.
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

- Only the coordinator may dispatch either workflow mode or create public
  tags/manifests. Development and review stop at `publication-ready`. After the
  private stage succeeds, the coordinator must pause and ask the organization
  owner to change exactly `genomoncology/pangopup` to public in GitHub's package
  settings. This is the sole required user action: the current coordinator PAT
  lacks package scopes, and GitHub documents only the package-settings UI for
  visibility. Finalization remains blocked until credential-free anonymous
  access proves that exact change.
- Package publication must target `genomoncology/pangopup`; avoid similarly
  named personal namespaces.
- A public failure after a manifest is visible is recorded honestly and stops
  work. Do not delete or mutate an observed public version tag to hide a failed
  qualification.
- The harmless Apple Docker ONNX CPU-vendor warning is accepted. The image is
  native Linux ARM64 CPU inference; it does not use macOS MPS/Metal.
- The visibility checkpoint records the exact stage run ID and combined receipt
  artifact identity. Finalization never trusts hand-copied digests and never
  searches for the latest successful stage.

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
infrastructure. The reviewer accepted that design, but implementation then
disproved its remaining visibility assumption: official GHCR-specific guidance
says first publication is private and linked packages inherit access but not
visibility. No package exists yet, the coordinator PAT has no package scopes,
and GitHub exposes visibility through package settings rather than a supported
API. The coordinator therefore revised the ticket to a two-mode stage/manual
visibility/finalize lifecycle and returned it to this reviewer.

The reviewer then required a run-authenticated handoff rather than trusting
copied digest strings. The accepted revision makes stage create one canonical
combined receipt after both native qualifications, makes finalization
authenticate the exact successful stage run/workflow/mode/commit and unique
retained artifact through the Actions API, and derives both digests only from
that receipt. The final two-phase design is accepted with no remaining material
findings.

## Implementation Evidence

Developer: independent implementation agent `ticket051_implementation`.

- Added `.github/workflows/publish-container.yml` as a manual, fixed-concurrency
  two-mode workflow. `stage` authenticates the exact current-main source,
  builds one native unattested leaf on each established runner, pushes and
  qualifies the held private digests, and emits one canonical two-leaf receipt.
  `finalize` authenticates the exact successful stage run, its successful stage
  jobs, and its unique unexpired receipt through the Actions API; requalifies
  both exact leaves anonymously on native runners with no package permission;
  and only then creates and verifies the OCI index and three tags.
- Extended `scripts/qualify-container.sh` with an optional exact registry
  digest. The existing four-argument local/CI contract remains unchanged; the
  fifth argument requires a digest-addressed image reference and the one held
  `RepoDigests` identity before all existing metadata, inventory, CLI, lookup,
  model, cache, and size checks run.
- Added static workflow, permission, action-pin, receipt, tag-collision,
  no-attestation, public-evidence, and held-digest checks to
  `tests/container-delivery.sh`, including a runtime negative control for an
  invalid expected digest. Updated the executable container and README specs.
- Replaced the local-build-first README path with the public versioned pull,
  named-volume sync/status/lookup/service/curl path, digest pinning, update,
  selective removal, retained exact-source local-build fallback, and unchanged
  Apple CPU-only boundary. Updated delivery/service architecture and the
  rolling frontier without changing the Dockerfile or runtime behavior.
- Added the credential-free, two-checkpoint coordinator record at
  `planning/artifacts/051-public-container.md`. It authenticates green gates,
  dispatches exactly one stage, stops at the explicit owner-only package
  visibility URL, then independently reloads the exact stage receipt and uses
  a fresh empty Docker configuration before a possible finalize dispatch.
- A live read-only API probe found that GitHub currently returns `inputs: null`
  for completed `workflow_dispatch` runs. The implementation does not rely on
  that unavailable field: it authenticates the exact workflow path/head and
  successful stage/finalize job sets through the Actions API, while the
  canonical receipt binds `mode`, run ID, commit, workflow SHA, and both leaf
  digests. No ticket assumption or security outcome changed.
- Focused evidence passed: `bash tests/container-delivery.sh`; actionlint 1.7.7
  on the new workflow; a PyYAML structural parse; `bash -n` for the helper,
  static test, and both extracted coordinator runbooks; `mustmatch test
  spec/container-image.md spec/readme-first-use.md` (`9 passed, 1 skipped`, the
  Docker-dependent scenario); and `git diff --check`.
- No workflow was dispatched; no GHCR login, push, package visibility change,
  tag, manifest, commit, or other external/public effect occurred.
- Code-review remediation removed persistent staging tags by switching the
  native exporter to `push-by-digest=true`; both collision gates now use an
  authenticated GHCR manifest request and accept only HTTP 404 with the exact
  `MANIFEST_UNKNOWN` error, with the final invocation directly adjacent to
  manifest creation. A shared helper authenticates the unique Actions artifact
  ID and canonical API `sha256:` digest, hashes the downloaded ZIP before
  extraction, rejects unsafe/noncanonical receipt archives, and is used by the
  workflow and both coordinator runbooks. Anonymous final verification now
  reads back all four index annotations. New negative tests cover missing,
  duplicate, malformed, expired/corrupt artifact inputs and 200, 401, 500,
  malformed-404, and disguised-denial tag responses; static tests reject any
  staging tag or ordinary `docker push` path and enforce the final collision
  gate's placement. Both authenticated collision requests advertise OCI image
  index, Docker manifest list, OCI image manifest, and Docker schema-2 image
  manifest media types, so an existing tag cannot hide behind its object kind.

## Adversarial Code Review

Reviewer: independent code reviewer `ticket051_code_review` — ACCEPT.

Initial review rejected persistent staging tags, collision probes that treated
all registry failures as absence, omission of GitHub's Actions-artifact digest
from the cross-run trust bridge, and index annotations that were written but
not read back. Remediation switched to true push-by-digest leaves, added a
fail-closed authenticated tag-absence helper and adjacent final check, added a
shared receipt-admission helper that verifies the API digest before extraction
with malformed/corrupt/duplicate/expired negative tests, and anonymously
verifies all four final index annotations.

Re-review found that the collision request advertised only index media types,
which could miss an existing single-platform image tag. Both probes now
advertise OCI index, Docker manifest list, OCI image manifest, and Docker
schema-2 image manifest, with an exact static regression. Final re-review
accepted the complete diff and focused evidence with no remaining findings.

## External Effect Evidence

Coordinator: pending. This ticket includes reviewed public GHCR publication and
must stop at `publication-ready` until the exact commit is pushed and its
required remote gates are green.

## Coordinator Final Check

Coordinator: Codex. Final `make lint`, `make test`, and `make spec` passed
(`268 passed, 7 skipped`), as did `git diff --check`. The stale-claim scan found
no remaining statement that the registry image is absent or that users must
build locally; the publication-ready README intentionally describes the
eventual exact image so the source used for public container labels and help is
not permanently stale after finalization. The accepted preparation is ready to
commit and push. No stage, package, tag, manifest, or visibility effect has yet
occurred.
