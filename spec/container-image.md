# Thin container image

The checked container delivery contract remains Dockerfile-first and has no
registry publication authority.

```bash
bash tests/container-delivery.sh
```

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
