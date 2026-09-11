---
---
# cargo clean fails on directories the qualification harnesses leave read-only

Ticket 0099 moved the downloaded ONNX Runtime library out of `target/` so that
`cargo clean` stops costing an 87 MB download. `cargo clean` is the routine
command that argument rests on. In this checkout it does not finish.

Measured on 2026-09-11, on a tree where `make test` and `make spec` had both
been run:

    $ cargo clean
    error: failed to remove directory
    `.../target/production-release-qualification-test/data/pangopup/bundles/qualified/bundle`
    Caused by:
      Permission denied (os error 13)

The directory is mode 555. `tests/production-release-qualification.sh` creates
it that way on purpose -- a bundle directory the release cannot write is part of
what the harness qualifies -- at lines 61 and 68, and never restores the mode.
Line 12 runs `chmod -R u+w "$root"` at the start of the *next* run, so the
harness recovers from its own residue and leaves it standing for everything
else. The shape predates ticket 0099: the same two `chmod 555` calls stand at
lines 56 and 63 of the file on `origin/main`.

Removing that residue by hand exposes a second family with the same cause:

    $ cargo clean
    error: failed to remove file
    `.../target/spec/local-assets/data/bundles/<digest>/receipt.json`
    Caused by:
      Permission denied (os error 13)

Four more 555 directories, under `target/spec/gene-naming` and
`target/spec/local-assets`, left by the spec suite.

`cargo clean` exits 101 both times, and it exits after deleting part of
`target/`, so the operator is left with a half-cleaned build directory and a
command that cannot be rerun to finish the job. `chmod -R u+w target` clears it.

What a successor must prove: `cargo clean` succeeds on a tree where every gate
has run, and the harnesses still create the unwritable directories they need.
A harness that creates one restores the mode before it exits, or the check that
holds this states which directory is allowed to stay read-only and why.
