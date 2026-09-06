# PangoPup v0.4.1 release notes

## Highlights

PangoPup v0.4.1 fixes complete removal of a normal read-only managed installation. `pangopup uninstall --full --yes` now establishes a private removal boundary, removes the managed data and cache, and removes the executable last. A failed removal preserves the executable. A permission-restore failure keeps the writable survivor detached behind a blocker for reviewed recovery.

Release qualification now validates the shipped HTTP response recursively with exact JSON types, permits a reviewed descendant of a staged release commit to run finalization without weakening the staged commit binding, and measures warmed-query allocations only on the measured thread. These changes remove false qualification failures across Linux and macOS.

## Compatibility

The HTTP, JSON, command-line, and scoring contracts do not change from v0.4.0 except for the reported software version and its derived scoring identity. Scoring assets do not change. Existing `snv-grch38-v1` and `runtime-grch38-v1` installations remain compatible and reusable through `pangopup sync --offline`.

## Install

The immutable Linux x86-64 installer is:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.4.1/install.sh \
  | bash -s -- --version 0.4.1
```

The native Linux AMD64 and ARM64 container image is:

```bash
docker pull ghcr.io/genomoncology/pangopup:0.4.1
```

The executable and container remain thin. Run `pangopup sync` against the separately versioned immutable scoring assets before scoring.
