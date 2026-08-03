# Ticket 042 — minimal container image evidence

## Built boundary

The root Dockerfile builds the locked `pangopup-cli` package in the pinned
Rust 1.93.1 Debian 13 image and copies the stripped executable, `LICENSE`, and
the corrected root `NOTICE` into the pinned Debian 13 distroless `cc` runtime.
The allowlisted build context excludes Git metadata, `target`, Python
environments, source datasets, fixtures, and generated assets. The final image
contains empty private data/cache mountpoints but no Dockerfile `VOLUME`.

The immutable SNV bundle notice is now sourced from
`assets/notices/SNV-BUNDLE-NOTICE-v1`. Its bytes and digest are unchanged, so
the already-published SNV asset and checked miniature remain readable while
the root product notice accurately describes the separately published model
release.

## Local native AMD64 qualification

The final image built successfully from the repository and the bounded helper
passed on the native AMD64 host:

```text
architecture=amd64
image_size=55,564,827 bytes
configured_user=65532:65532
entrypoint=["/usr/local/bin/pangopup"]
command=["serve","--listen","0.0.0.0:8080"]
qualification=passed
```

The helper inspected the final config and exported filesystem, rejected any
shell/package manager/build tree/Git metadata/model or index member, exercised
the exact missing status and default-startup failure, installed the miniature
SNV through the real transport/install command, returned an installed mmap
lookup, ran miniature ONNX inference twice, copied the created SQLite database
out of its named volume after each call, and proved the second call left the
database byte-identical. Every
runtime invocation used the configured non-root user, a read-only root,
network isolation, a bounded `/tmp` tmpfs, and fresh named data/cache volumes.
The helper removed only its uniquely named container, volumes, and scratch
directory.

The 55,564,827-byte Docker engine content size is below the 78,643,200-byte
limit. It is not a compressed registry-transfer measurement.

## Two-architecture and production authority

`.github/workflows/container.yml` performs the same native final-image smoke
on `ubuntu-24.04` and `ubuntu-24.04-arm` with `contents: read` permission and no
login, push, artifact upload, or package permission. Its manual exact-commit
matrix downloads exactly the ten `runtime-grch38-v1` transport members
(691,874,664 stored bytes), checks every declared size and SHA-256, reconstructs
only the model/reference/mask tuple, makes the authenticated decoded tree
read/traverse-only for the numeric image user, and mounts that tuple read-only.
It sends all 14 variants in one ordered model-only call through each final image
and compares every line's provenance plus the ordered result bodies to the
checked oracle.

The developer did not run the 691 MB manual production download or an ARM64
job locally. Those results remain workflow evidence; the workflow cannot
publish an image. No registry or release was mutated by this ticket.

## Code-review remediation

The corpus-derived oracle now preserves an already-negative loss score and has
an explicit `M08-mnv-both-strands` `-0.08` sentinel; only signed zero is
normalized. Production qualification keeps the downloaded transport private,
changes only the authenticated decoded tree to directories `0555` and files
`0444`, then mounts the explicit tuple read-only. Its comparator requires the
exact provenance on every one of the 14 lines and compares the ordered bodies.
The production cache volume is mounted at the image-prepared
`/var/cache/pangopup`, so numeric UID/GID `65532:65532` can create the default
SQLite file.

The first post-push native smoke exposed a portability problem in an early
silent metadata assertion on both runners. Metadata checks now report the
stage, named field, expected value, and observed value, and exposed ports are
compared as canonical Docker JSON (`{"8080/tcp":{}}`) instead of relying on a
Go-template rendering of the indexed empty value. A deliberate local revision
mismatch proved the actionable diagnostic, while a fresh no-cache image passed
the complete native AMD64 qualification.

The corrected read-only workflow run
[`30827959840`](https://github.com/genomoncology/pangopup/actions/runs/30827959840)
then passed the complete final-image smoke natively on both
`ubuntu-24.04` (AMD64) and `ubuntu-24.04-arm` (ARM64). The manual production
matrix was correctly skipped on the ordinary push; no image or asset was
published.

Manual workflow run
[`30828329814`](https://github.com/genomoncology/pangopup/actions/runs/30828329814)
subsequently passed that ordinary smoke again on both architectures. Its two
production jobs downloaded and authenticated the runtime transport and
completed all 14 inference cases inside the native final images. Neither job
qualified the result: the host helper called `jq -e --slurpfile expected`
without the required oracle filename, so jq treated the filter as that
filename and exited before executing the exact comparison.

The reviewed correction binds the checked oracle path once and passes it both
to request construction and explicitly as `--slurpfile expected "$oracle"`.
The static container-delivery contract now checks that invocation, and a
focused local jq exercise compared a synthesized 14-line stream against the
real oracle successfully. This is correction evidence only. Production
qualification remains pending until the corrected exact commit passes the
native AMD64 and ARM64 manual matrix; run `30828329814` must not be cited as a
production-compatible result.

Corrected exact-commit workflow run
[`30829221866`](https://github.com/genomoncology/pangopup/actions/runs/30829221866)
resolved that pending check. It checked
`423e806cf8577488c71fd95403ab9b37b7f02d90`; both native production jobs
authenticated the runtime transport, executed all 14 retained model cases,
and passed the exact ordered result and provenance comparison. The native
AMD64 and ARM64 miniature smoke jobs also passed. No image or asset was
published. This run is the production-compatible container evidence for
Ticket 042.

The ordinary native smoke copies `model-results.sqlite3` from the named volume
after the first and second calls, checks the SQLite header, and requires the two
database files and two public outputs to be byte-identical. The production
runtime-only path does not claim installed-profile coverage; the existing
installer qualification remains that boundary's authority.

## Checked gates before review

```text
cargo test --locked -p pangopup-cli --test container_oracle: passed (1 test)
bash tests/container-delivery.sh: passed
bash -n scripts/qualify-container*.sh tests/container-delivery.sh: passed
docker build: passed
scripts/qualify-container.sh: passed (native AMD64)
make lint: passed
make test: passed
make spec: passed (246 passed, 7 skipped)
```

The normal gate remained offline and did not build Docker or fetch production
assets. Regenerating the checked miniature SNV fixture changed only its builder
provenance and derived bundle identity; `scores.pgi`, the frozen notice, source
inputs, requests, and reference bytes remained unchanged. Historical and
production asset identities were not changed.
