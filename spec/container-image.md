# Thin container image

The checked container delivery contract remains Dockerfile-first and has no
registry publication authority.

```bash
bash tests/container-delivery.sh
```

The bounded final-image qualification installs the checked miniature SNV
through the shipped CLI, then runs network-disabled `pangopup status` as the
image's non-root user with a read-only root filesystem and
`/var/lib/pangopup:ro`. The expected state is `partial`; full fixture-only
combined readiness remains a crate-level proof and is not exposed as a
production CLI bypass.
