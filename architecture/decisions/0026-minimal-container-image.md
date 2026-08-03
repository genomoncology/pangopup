# ADR 0026: Minimal qualified container image

Status: accepted

## Decision

Ship one multi-stage Dockerfile as the first container primitive. It builds the
locked `pangopup` executable natively on Linux AMD64 or ARM64, strips it, and
copies it with the GPL license and notices into a pinned Debian 13 distroless
image. The process runs as numeric UID/GID `65532:65532`; its default command
is the foreground HTTP service on `0.0.0.0:8080`.

The image never embeds the SNV index, model, compact reference, or splice mask.
Operators explicitly run `pangopup sync` with named data and cache volumes,
then mount the data volume read-only for lookup or service use. The Dockerfile
does not declare `VOLUME`: forgetting a mount must fail visibly instead of
silently creating an anonymous multi-gigabyte volume.

## Why

Embedding immutable runtime assets would make a software upgrade duplicate
roughly 16 GB and couple image publication to independently versioned data.
An implicit download entrypoint would make service startup network-dependent.
The existing explicit sync and XDG-style path boundaries already provide the
smaller, clearer composition.

Debian 13 is required because the selected static ONNX Runtime imports C23
glibc symbols not present in Debian 12. Distroless removes the shell, package
manager, and toolchain from the runtime image. Native two-architecture builds
avoid treating emulation or successful linking as numerical qualification.

## Qualification and exclusions

Every ordinary container job exercises the final image as non-root under a
read-only root filesystem with checked miniature lookup and model assets,
explicit writable volumes, and SQLite reuse. A manual exact-commit matrix may
download only the authenticated runtime transport and must match all 14 frozen
Pangolin public results through the final stripped image on each native
architecture.

This decision does not add Compose, a Dockerfile healthcheck, restart policy,
registry publication, signing, SBOM/attestation, Kubernetes, systemd, TLS,
authentication, or accelerator support.
