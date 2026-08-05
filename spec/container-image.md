# Thin container image

The checked container delivery contract keeps the ordinary Dockerfile and
push/PR workflow read-only. A separate exact-commit, manually dispatched
two-mode workflow stages native digest-addressed leaves without user-facing
tags, stops at a GHCR public-visibility checkpoint, anonymously requalifies the held child digests,
and only then assembles the public two-platform index.

```bash
bash tests/container-delivery.sh
```

The final image remains thin: neither a leaf nor the OCI index contains the
SNV, model, reference, mask, or SQLite data. The human version tags are
`0.3.0` and `v0.3.0`; the manifest digest is the immutable deployment identity,
while `latest` is explicitly moving. The workflow's canonical stage receipt,
not a hand-copied digest or a latest-run search, is the only handoff into
finalization. Native staging uses an explicitly selected Docker-container
Buildx builder with an immutable BuildKit image, exports each leaf by registry
digest, and leaves no staging tag. Receipt admission hashes the downloaded
Actions archive against its API digest before extraction, and tag collision
checks accept only an authenticated `MANIFEST_UNKNOWN` response.

A stage receipt is usable only with the exact source commit and workflow SHA
that created it. If publication code changes after staging, recovery starts
with a new exact-commit stage and a new stage run ID; an earlier receipt is
abandoned rather than rebound or admitted under weaker checks. Digest-only
leaves from an abandoned attempt may remain untagged, but they are not the
versioned image and cannot substitute for a qualified two-platform index.

Before touching any volume, the bounded final-image qualification runs all
eight non-root runtime help paths with both `-h` and `--help`. Every invocation
runs with `--network none`, a read-only root filesystem, the image's numeric
non-root user, and no data or cache mount. It must exit successfully, emit the
path's expected leading `Usage:` line, and emit no PangoPup JSON error; any
third-party pre-main stderr remains outside this contract.

The bounded final-image qualification installs the checked miniature SNV
through the shipped CLI, then runs network-disabled `pangopup status` as the
image's non-root user with a read-only root filesystem and
`/var/lib/pangopup:ro`. The expected state is `partial`; full fixture-only
combined readiness remains a crate-level proof and is not exposed as a
production CLI bypass.
