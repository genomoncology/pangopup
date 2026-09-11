---
base: d0d8022999345390a3b2095678ae672a948021ad
head: 65432771e642a734d5ff6aaad707152e9bad85a0
---
# The shell qualification harnesses reach the executable unwatched

A shell file that names the built `pangopup` executable now holds a cache home
of its own. `tests/support/private-cache-home.sh` establishes it: one sourced
helper that moves `HOME` and `XDG_CACHE_HOME` to a fresh directory under
`target/`, pins `CARGO_HOME` and `RUSTUP_HOME` to where they resolved before
the move, and refuses to be run rather than sourced.
`tests/shell-spawn-cache-isolation.sh` enforces both sides: that every shell
file naming the executable sources the helper before its first run, and that
the helper redirects. The three harnesses that reach the executable —
`tests/executable-delivery.sh`, `tests/production-release-qualification.sh` and
`tests/release-help-contract.sh` — source it after their build and before their
first run.

The rule reads names, not argument shapes. A file that names the executable
holds a cache home, whether or not the arguments on a given line happen to
resolve a default cache. The scan counts what it read, so a pattern that stops
matching fails rather than passes, and the refusal names the file and the line.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME` and
`TMPDIR`, with `CARGO_HOME` and `RUSTUP_HOME` left on the real ones, as three
contributor mistakes made against the real tree.

A new `tests/contributor-probe.sh` that builds and then runs
`target/debug/pangopup` with no cache home was refused: `this harness runs the
built executable without a cache home of its own, so it reaches the model cache
of whoever runs it and can discard that person's cache:
tests/contributor-probe.sh:5`, followed by `source
tests/support/private-cache-home.sh before that line, with a leading `.` or
`source``.

Moving the existing source point in `tests/executable-delivery.sh` to the end
of the file was refused: `this harness runs the built executable at
tests/executable-delivery.sh:83 but takes its cache home at line 624, so the
runs before that line reach the model cache of whoever runs it`. The
inheritance rule refused the same file in the same run, for handing an
executable to `scripts/smoke-linux-release.sh` with no cache home to hand it.

Moving the source point in `tests/release-help-contract.sh` ahead of its build
was refused twice, once for each half of the ONNX constraint: `this harness
takes its cache home at tests/release-help-contract.sh:11 and then builds at
line 12: the ONNX Runtime library the build resolves lands under
XDG_CACHE_HOME, so a build under a fresh cache home downloads it again instead
of linking the copy already there. Build first, take the cache home after.` and
`this harness builds and runs the executable in one step at
tests/release-help-contract.sh:14 under the cache home it took at line 11, and
nothing builds before that line`.

Each probe was removed and the tree left clean.

The three harnesses still exercise their assertions rather than passing under a
moved `HOME`. Mutating the `lookup --help` pin in `spec/cli.md` made
`tests/release-help-contract.sh` fail with `focused lookup help differs from
spec/cli.md`. Renaming the `unknown argument` rejection in `install.sh` made
`tests/executable-delivery.sh` fail. Changing one variant in
`tests/fixtures/snv-regression/requests.tsv` made
`tests/production-release-qualification.sh` fail with `request fixture identity
mismatch`, which is the check that renders through the shipped executable.

The operator's cache was fingerprinted before and after everything above.
`~/.cache/pangopup/model-results.sqlite3` read
`d0a9bfaab1ac1be8258774dee563f6ebfaf4254b0a72c66fbe7cc885c7133a35`, 303104
bytes, mtime 1788900083, at the start and the same three values at the end.
`target/shell-harness-cache-home` was created and held no `pangopup`
directory afterwards, which is the ticket's own finding: no current invocation
resolves a default model cache. The rule is what changed, not the accident.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
`bash tests/shell-spawn-cache-isolation.sh`, `bash
tests/cli-spawn-cache-isolation.sh`, `bash tests/built-executable-currency.sh`
and `bash sdlc/scripts/lint` all pass on this candidate. `make spec` reports
`306 passed, 6 skipped` and did not hit the fixed 30-second cap at
`spec/http-service.md:76`. `make test` took 83.1 seconds and then 80.9 seconds
warm. No base-commit comparison was taken, because checking out the base was
not available here, so no baseline figure is claimed. What was measured
directly is the new gate on its own: `bash
tests/shell-spawn-cache-isolation.sh` runs in 1.07 seconds, inside the
two-second budget its siblings carry, and it is the only work `make test`
gained.

Two costs are accepted rather than carried as open work.
`scripts/smoke-linux-release.sh` runs an executable handed to it and names
none, so no name scan can see it; what covers it is inheritance, checked rather
than asserted, and the check reads whether the caller takes a cache home
without reading whether it takes one before the hand-off. `make spec` protects
the operator through one `XDG_CACHE_HOME` assignment in the Makefile recipe
that no gate reads, so deleting that assignment would send the spec run at the
operator's cache with nothing refusing it.

Four drafts filed by the earlier stages stand: 0086, 0088, 0089 and 0090. One
more was filed here, 0091: `tests/executable-delivery.sh` ends several checks
in a bare command under `set -e`, so a failing assertion stops the harness with
no message naming what failed.
