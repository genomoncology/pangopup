---
base: 8a5734d4545e8194d5ba107434daac8deaf12d31
head: 037b31c1e8fcc65899e01c39bf6af7bdbbeae8a7
---

# Keep asset authority descriptor-relative on Linux and macOS

Local asset operations now hold their admitted directory and file descriptors across validation, iteration, copying, and publication on Linux and macOS. Path replacement cannot redirect a later operation, symlinks and cross-device members fail closed, and cleanup leaves a foreign replacement untouched.

Runtime status reads only bounded metadata and declared payload sizes. Full admission still authenticates payload content. The macOS CLI continues to refuse `status`, `assets install`, `sync`, and `serve` until ticket 0003 delivers the command, documentation, specification, and CI work.

The accepted implementation passed 116 asset tests, five macOS CLI-boundary tests, `make lint`, `make test`, and `make spec` on macOS. A later Linux gate exposed an unused non-macOS parameter in `sync.rs`, so this record does not claim a successful Linux gate. Ticket 0003 owns the portable compile correction and the new platform matrix. A Linux ARM64 container build also exposed an unrelated `st_nlink` type mismatch in `uninstall.rs`; the same failure reproduces at the pre-0002 base `8a5734d4545e8194d5ba107434daac8deaf12d31`.
