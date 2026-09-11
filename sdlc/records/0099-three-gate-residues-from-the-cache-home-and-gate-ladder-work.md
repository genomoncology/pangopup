---
base: 9d2b401db07aa598eba2748ad3dce4a3240b988f
head: 4a11ecb424a25b44e263ea8b560c9273ae42618c
---
# Three gate residues from the cache home and gate ladder work

`tests/recipe-spawn-cache-isolation.sh` now carries
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES` in `named_locations` and tells it apart from
the `PANGOPUP_MODEL_CACHE` inside it. The `spec` recipe points
`ORT_CACHE_DIR` at `$HOME/.cache/ort.pyke.io` rather than `target/ort-cache`,
and `tests/spec-download-cache-durability.sh` refuses the build directory
whether or not the recipe names it. `tests/built-executable-currency.sh` asks
where `scripts/require-built-commands.sh` stands rather than whether the text
appears. Sixty-six assertions in `tests/production-release-qualification.sh`
are `require_text`, `require_line`, `require_pattern` and `equal` calls, and
`tests/negative-assertion-strength.sh` reads that harness as a second named
file. Two new coverage harnesses, `tests/harness-rule-coverage.sh` and
`tests/recipe-cache-rule-coverage.sh`, join `PORTABLE_QUALIFICATION`.

Exercised beyond the suite with `CARGO_HOME` and `RUSTUP_HOME` left at the
operator's own, every `PANGOPUP_*` name unset except where the exercise
deliberately exported one, and a private cache root under `/tmp` for every run
of the product.

## The download, measured

The ticket's central claim was that `cargo clean` followed by `make spec`
reaches no download. It was measured, not asserted, and it holds.

`~/.cache/ort.pyke.io` was inventoried whole before the run: 11 entries,
288252072 bytes, three `libonnxruntime.a` files. `target/ort-cache` held
90647244 bytes. The digest the build resolves,
`acc1cba79c337594ead1d88ca72516147aa60054c84217b53399a31caa5ba671`, stood in
both, md5 `633d3b9c09f31df0d7078e45df3e4976` in both.

`cargo clean` removed all of `target/`, `target/ort-cache` with it. `make spec`
then ran cold: 1 minute 50.996 seconds, `314 passed`, exit 0. After it, the
inventory of `~/.cache/ort.pyke.io` was byte-identical to the one taken before
-- same 11 entries, same sizes, same modification times to the nanosecond, same
md5 for all three files. Zero bytes written, nothing removed, nothing
rewritten.

`ort-sys` logged no download. All three of its build-script outputs carry a
`cargo:rustc-link-search` into
`/home/ian/.cache/ort.pyke.io/dfbin/x86_64-unknown-linux-gnu/acc1cba7...` and
none carries the `[ort-sys] [DEBUG] downloading from` line that a fetch writes.
The machine received 7232271 bytes on its interface across the whole clean and
rebuild, against a 90647244-byte library.

`target/ort-cache` was not recreated by that run or by any run since. The
orphan is gone and one copy on the machine serves every gate.

## The entry limit, as a maintainer meets it

A throwaway recipe was appended to the `Makefile`, running the built executable
under `XDG_CACHE_HOME` and `HOME` in `$(CURDIR)/target` with `CARGO_HOME` and
`RUSTUP_HOME` pinned. Dropping the three older names and inheriting the entry
limit was refused:

    this recipe reaches the built executable with
    PANGOPUP_MODEL_CACHE_MAX_ENTRIES inherited, so the operator who exported it
    decides where that run keeps its model cache or how much of it the run may
    keep: Makefile:137

Dropping only the entry limit was refused three times, once for each of the
other names, so the longer name does not answer for the shorter one. Dropping
all four was accepted, and the scan reported 2 recipes examined rather than 1.

The refusal names the variable and the line the recipe stands on. It does not
name the recipe's own target. A maintainer reads `Makefile:137` and finds it;
naming `throwaway` as well would be better and is not what this ticket asked
for.

## The currency scan, as a maintainer meets it

A harness whose line 3 is `printf 'scripts/require-built-commands.sh\n'` and
which then runs `target/debug/pangopup` was refused:

    tests/zz-verify-mention-only.sh uses an executable from target/debug at
    line 4 without building it first, so a standalone run compares against
    whatever the last build left behind

A harness that assigns the path to `builder` and then runs `"$builder"` was
accepted, and the scan reported 5 harnesses reaching into `target/debug`, each
building first. A mention is no longer a build, and a call through a variable
still is.

## The converted assertions, against three real breakages

Each break was a change a release would really make, and each was reverted with
`git checkout` and the tree confirmed clean afterwards.

The admitted glibc ceiling in `planning/artifacts/038-public-linux-release.md`
was moved from `2.39` to `2.38`:

    expectation failed: a text a gate requires is not in the file it reads
      wanted: admitted maximum imported GLIBC version: `2.39`
      in:     .../planning/artifacts/038-public-linux-release.md

One of the four mentions of the pinned build image digest in the same record
was changed to zeroes:

    expectation failed: the number of times the release record names the pinned
    build image
      expected: 4
      found:    3

The published claim in `AGENTS.md` was reworded from `checksum-verifying tagged
installer are shipped` to `... ship`:

    expectation failed: a text a gate requires is not in the file it reads
      wanted: checksum-verifying tagged installer are shipped
      in:     .../AGENTS.md

Each names the expectation and what stood there instead. Before this candidate
each would have stopped the harness with status 1 and no line of its own.

## The product still reads the limit

Run against the miniature model kernel under a private cache root, with three
distinct modelled lookups at `GRCh38:chr1:5051` and a fresh cache file for each
setting: `PANGOPUP_MODEL_CACHE_MAX_ENTRIES=unlimited` left 3 rows, `3` left 3,
`2` left 2 and `1` left 1. `not-a-limit`, `0`, `-3` and an empty value were each
refused with exit 2 and `invalid model cache configuration: model cache maximum
must be a positive integer or unlimited`. The operator's own
`~/.cache/pangopup` was never written.

## Verify's own changes

Two commits, both in scope.

The `spec` recipe's comment claimed `ort-sys` resolves `$HOME/.cache/ort.pyke.io`
on Linux when nothing names one. `XDG_CACHE_HOME` names one first, and an
operator who exports it moves the copy `make lint` and `make test` fill while
the recipe keeps writing to the fixed path, so that operator pays a second copy
on Linux too. The comment says so now, carries the measurement above, and reads
as sentences rather than as clauses appended after a dash.

Draft 0112 holds a `cargo clean` that cannot finish. On a tree where the gates
have run it exits 101 on a mode-555 directory under
`target/production-release-qualification-test`, and after that is cleared, on
four more under `target/spec`. `tests/production-release-qualification.sh`
creates its own at lines 61 and 68 and only restores the mode at the start of
its next run; the spec suite leaves the others. The shape predates this ticket
-- the same two `chmod 555` calls stand on `origin/main` -- but the command this
ticket's central claim rests on is the command that fails, so it is named.
`chmod -R u+w target` clears it, and that is what verify did before taking the
measurement above.

## The intermittent gate, measured rather than estimated

`tests/shell-spawn-cache-isolation.sh` is not repaired here. Draft 0111 holds
it and now holds a number: 40 runs on a 16-core machine carrying 24 busy loops,
7 failures, 17.5 per cent. Six refused from `runs_an_argument` and two from
`hands_over`, which is the second site draft 0111 predicted and nothing had
observed. One run printed both. The gate examined the same 53 shell sources in
every run, passing and failing alike. The file is untouched by this candidate.

## The ladder

`make lint` (31.26 s), `make test` (112.75 s warm, 138.69 s on the cold run
straight after `cargo clean`), `make spec` (13.99 s warm, `314 passed`),
`scripts/run-service-fixture-tests.sh`, `bash sdlc/scripts/lint`, and all 26
`PORTABLE_QUALIFICATION` and 3 `SHELL_QUALIFICATION` gates run one at a time,
all pass. `ORT_CACHE_DIR` was never exported.

`make test` grew about 9 seconds against the ~104-second baseline, past the two
seconds the ticket budgeted. Draft 0109 already holds it and names the cause:
`tests/recipe-cache-rule-coverage.sh` at 2.30 s and
`tests/harness-rule-coverage.sh` at 0.47 s, and a `python_holds` scan that
reads every `*.py` in the checkout. The slowest gate is
`tests/recipe-spawn-cache-isolation.sh` at 21.43 s, unchanged by this ticket.

Reported counts: `tests/recipe-spawn-cache-isolation.sh` 1 recipe, 24 spec
files, 1 Python file; `tests/spec-download-cache-durability.sh` 1 recipe;
`tests/built-executable-currency.sh` 4 harnesses; `tests/negative-assertion-
strength.sh` 8835 statements in 53 shell files, 0 bare, 22 forbidden texts, 96
assertions in 2 harnesses; `tests/harness-rule-coverage.sh` 70 assertions and
57 recorded expectations; `tests/model-cache-limit-inheritance.sh` 4 routes, 4
variables, 12 matcher cases, 7 runs.

The operator's own `~/.cache/pangopup/model-results.sqlite3` is byte-identical
after all of it: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, 303104 bytes, mtime
and inode unchanged.
