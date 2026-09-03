---
flow: build
priority: 6
---
# Asset installation runs on macOS through one code path, not two

`pangopup sync`, `pangopup assets install`, `pangopup status`, `serve` and the
runtime install and release paths all refuse on macOS today. Twenty entry
points across `pangopup-assets` call `require_linux()` and return
`UnsupportedPlatform`. A developer on a Mac cannot do a first run at all.

The refusal is not protecting anything that macOS cannot protect. `component()`
in `crates/pangopup-assets/src/local.rs` already rejects an empty name, `.`,
`..`, and any name containing `/` before an open happens. Every open is
therefore one plain name, one level below a directory the process already holds
open. The kernel resolution flags being requested guard against escapes that a
single non-`..` component cannot perform, against magic links that macOS has no
equivalent of, and against crossing a mount point, which is observable from a
file's own metadata on both systems.

Directory iteration and no-replace rename have the same shape. One of the three
already has a working macOS spelling in this repository, in
`crates/pangopup-build/src/runtime_profile.rs`.

## The provenance change is authorized

A previous attempt refused this ticket because its boundary forbade touching
the files `crates/pangopup-build/src/source_fingerprint.rs` hashes with
`include_bytes!`, and the required spec files cannot run without them. That
refusal was right and the boundary was wrong.

Editing those files is authorized here. `snv_source_sha256()` reaches a
published bundle at `crates/pangopup-build/src/production.rs` as
`builder.source_sha256`, so a bundle built after this ticket will declare a
different builder source hash than one built before it. That is what the hash
is for. The two pins in `source_fingerprint.rs`, `EXPECTED_SNV_SHA256` and
`EXPECTED_REFERENCE_SHA256`, are re-pinned to the values the changed sources
produce. Recompute them; never copy a value from a failing test's output
without checking that the only inputs that changed are the ones this ticket
changed.

Nothing is published by this ticket.

Done, observably:

- `pangopup sync`, `pangopup assets install` and `pangopup status` complete on
  macOS against a local transport, and produce the same result they produce on
  Linux for the same input.
- The spec files currently excluded from the macOS run for asset reasons run on
  macOS: `local-assets.md`, `remote-assets.md`, `runtime-install.md`,
  `runtime-release.md`, `runtime-transport.md`. Remove them from the Makefile's
  macOS exclusion list rather than leaving them listed and passing.
- A test plants a symlink inside the asset directory pointing outside it, and
  the install refuses to follow it. That test runs and passes on both platforms.
- A test plants a name that would cross a mount point, and the install refuses
  it. That test runs and passes on both platforms.
- The change removes more lines than it adds in `crates/pangopup-assets/src/`,
  measured by `git diff --stat` on that directory alone.
- No `#[cfg(target_os = ...)]` split remains in `crates/pangopup-assets/src/`
  except where no primitive exists that behaves the same on both systems. Each
  split that survives carries a comment naming the primitive that has no
  portable equivalent.
- `make lint`, `make test` and `make spec` exit 0 on macOS. The Linux CI job
  stays green.

The baseline to measure against, taken at this ticket's commit: 76 occurrences
of `target_os` across `local.rs`, `sync.rs`, `runtime_release.rs`,
`runtime_transport.rs` and `release.rs`; those five files total 12,625 lines.

Boundary. In `crates/pangopup-build/`, change only what a platform refusal
forces: the no-replace rename primitive, and the two re-pinned constants. Leave
what the builder computes, reads, writes and certifies exactly as it is. A
bundle built on Linux before and after this ticket must differ in
`builder.source_sha256` and in nothing else.

Do not weaken any check to make a platform pass. If a guarantee genuinely
cannot be reproduced on macOS, keep the refusal for that one path, say in the
code comment which guarantee is missing, and leave the rest working. Do not
change the CLI's `uninstall` path, which is Linux-only for its own reasons, and
do not change how bundles are verified or scored.
