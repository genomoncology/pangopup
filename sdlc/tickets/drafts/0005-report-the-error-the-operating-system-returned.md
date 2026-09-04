---
flow: build
priority: 7
---
# An asset failure reports the error the operating system returned

A local asset failure that reports an operating-system error reports the wrong
one. A data directory that is a regular file, and a data directory the user may
not read, both answer:

```
{"code":"ASSET_IO","message":"inspect data root: Bad address (os error 14)"}
```

The operating system returned `ENOTDIR` for the first and `EACCES` for the
second. "Bad address" describes neither, and comes from an unrelated call that
runs afterwards. Under strace:

```
statx(AT_FDCWD, ".../pangopup", ...) = -1 ENOTDIR (Not a directory)
statx(AT_FDCWD, ".../pangopup", ...) = -1 EACCES  (Permission denied)
statx(0, NULL, AT_STATX_SYNC_AS_STAT, STATX_ALL, NULL) = -1 EFAULT (Bad address)
```

A wrong data directory, a permissions problem, a read-only filesystem and a full
disk are the failures an operator meets while installing for the first time. All
of them report an error that points nowhere, so the message cannot start a
diagnosis and can only mislead one.

This is not every failure in the file. `crates/pangopup-assets/src/local.rs`
already formats the caught error correctly in several places, and its semantic
failures — the ones reporting a rule the assets broke rather than a call that
failed — are right as they stand and are not in scope. The defect is confined to
the paths that catch an `io::Error`, discard it, and then report an error read
back from elsewhere. `crates/pangopup-cli/src/uninstall.rs` shows the correct
pattern, so both behaviors ship in the same executable today.

Issue: `sdlc/issues/2026-09-04-report-the-error-the-syscall-returned.md`

Done, observably:

- A local asset operation that fails on an operating-system call reports the
  error that call actually returned.
- Two different underlying failures produce two different messages, and each
  names its own cause.
- An operator can tell a wrong path from a permissions problem by reading the
  message.
- The message still names which operation was attempted, as it does today.
- The suite pins at least one such failure by cause, with a case that fails
  before the change.

Boundary: this changes the text a failure carries and nothing about which
operations fail, when they fail, what error kind or code they report, or what
the process exits with. Do not change the semantic failures, which report a rule
rather than a call and are already correct. Do not change the paths in this file
that already format their caught error. Do not change the reporting in
`uninstall.rs`, which is already correct. Do not change the sync-side reporting, which does not have this
defect. Do not widen what any message discloses beyond the failure's own cause.
