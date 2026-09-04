# Report the error the failing call returned

## Observation

`asset_io` in `crates/pangopup-assets/src/local.rs:2113` builds its message from `io::Error::last_os_error()` rather than the error it was handed:

```rust
fn asset_io(action: &str) -> AssetError {
    AssetError::new(
        AssetErrorKind::AssetIo,
        format!("{action}: {}", io::Error::last_os_error()),
    )
}
```

Every caller discards the real error first. `open_root` at line 1220 is one of 61 such sites in the file:

```rust
Err(_) => return Err(asset_io("inspect data root")),
```

By the time `last_os_error` runs, a later syscall has replaced `errno`. Two different failures both report a third, unrelated error:

```
$ XDG_DATA_HOME=/etc/hostname pangopup status
{"code":"ASSET_IO","message":"inspect data root: Bad address (os error 14)"}

$ XDG_DATA_HOME=<unreadable dir> pangopup status
{"code":"ASSET_IO","message":"inspect data root: Bad address (os error 14)"}
```

strace shows the real results and the clobbering call:

```
statx(AT_FDCWD, ".../pangopup", ...) = -1 ENOTDIR (Not a directory)
statx(AT_FDCWD, ".../pangopup", ...) = -1 EACCES  (Permission denied)
statx(0, NULL, AT_STATX_SYNC_AS_STAT, STATX_ALL, NULL) = -1 EFAULT (Bad address)
```

`EFAULT` is what the caller always sees.

## Why this matters

A wrong data directory, a permissions problem, and a full or read-only filesystem are the failures an operator actually hits during a first install. All three report "Bad address", which describes none of them and points nowhere. `crates/pangopup-cli/src/uninstall.rs:701` already formats the caught error directly, so the correct pattern exists in the tree.

## Suggested direction

Give `asset_io` the caught `io::Error` and format that. The 61 `map_err(|_| ...)` sites become `map_err(|error| ...)`. A test that sets the data root to a regular file and asserts the message names `ENOTDIR` would keep it fixed.
