---
base: e88af527591950e3bc2ba52cf975afb5b3ad08a9
head: fbca2d0c9966365824cd54b14a2fee7ef6405ef5
---
# Routes to the executable that no cache-home rule holds

Both spawn helpers now drop `PANGOPUP_MODEL_CACHE`, `PANGOPUP_CACHE_DIR` and
`PANGOPUP_DATA_DIR` on the way in. `tests/support/private-cache-home.sh` unsets
them after moving `HOME` and `XDG_CACHE_HOME`, and
`crates/pangopup-cli/tests/support/mod.rs` removes them from the command it
builds. They are removed rather than emptied, because an empty value is still a
value the product reads, and they are dropped at helper time rather than at
spawn time, so a caller that names a cache location of its own afterwards still
reaches the child with it.

The `spec` recipe moves `HOME` beside `XDG_CACHE_HOME`, drops the same three,
and pins `CARGO_HOME` and `RUSTUP_HOME` to what they resolved to before the
move. Without that pin a rustup shim would install the toolchain from the
network into a directory the recipe deletes.
`maintainers/ticket-053/measure.py` hands every child an explicit environment
from `child_environment` instead of inheriting the maintainer's, and passes the
measurement cache through `PANGOPUP_MODEL_CACHE` rather than `--model-cache`,
so the drop is what the run proves.

Three checks hold it. `tests/inherited-cache-variables.sh` runs with a variable
exported at a file it then finds untouched.
`tests/recipe-spawn-cache-isolation.sh` reads the `Makefile`, the spec blocks
and the Python that no `*.rs` or `*.sh` scan can see, and counts what it read.
`tests/shell-spawn-cache-isolation.sh` now reads `cargo run --package
pangopup-cli` with no `--bin`, long and short, as the run it is.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME`,
`XDG_DATA_HOME` and `TMPDIR`, with real `CARGO_HOME` and `RUSTUP_HOME`. An
operator's scenario was run whole: a copy of a real 303104-byte cache placed in
a private directory, `PANGOPUP_MODEL_CACHE` exported at it,
`PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR` exported at directories beside it,
then `make test`, `make spec` and `tests/production-release-qualification.sh`
run by hand. All three passed. The named file kept md5
`0ce402e99f91b4d16ed3aaddc88160fc`, cksum `2360715374 303104`, size 303104 and
mtime, and the two named directories stayed empty. The real cache at
`~/.cache/pangopup/model-results.sqlite3` carried the same fingerprint before
and after every command in this flight.

The variables still do what an operator asks. With `PANGOPUP_MODEL_CACHE`
exported at a fresh path under a private `HOME` and `XDG_CACHE_HOME`, a
`--model-only` lookup created that file, wrote one entry, served the second run
from it, and left nothing under the private cache home. A shell that exported
an inherited value, sourced the helper and then exported one of its own reached
its own file and not the inherited one.

Three contributor probes were written and refused. A new `Makefile` recipe
putting `target/debug` on `PATH` drew seven refusals naming `Makefile:88`, one
for each rule. A spec block clearing `XDG_CACHE_HOME`, and another pointing it
somewhere relative, were each refused by file and line. A new Python file under
`maintainers/` running `target/release/pangopup` drew five refusals, one for
each name it never hands the child. Each was removed after it refused.

`make spec` on a machine with neither `CARGO_HOME` nor `RUSTUP_HOME` exported
downloads no toolchain. The installed rustup toolchains were identical before
and after, no `.cargo` or `.rustup` appeared under `target/spec-cache`, and the
recipe's own output carried no rustup install line. `target/spec-cache` holds
20480 bytes, its private model cache and nothing else, and the run took 13.03
seconds.

One cost is accepted rather than carried as open work. `specs_hold` reaches
only the spec blocks that name a cache home, one block across 24 files today,
because a block that names neither runs under the homes the recipe already
took. The rule prints both counts, so a pattern that stops matching shows up as
a zero rather than as a pass.

`make lint`, `make test`, `make spec`,
`scripts/run-service-fixture-tests.sh`, all fourteen
`PORTABLE_QUALIFICATION` gates run one at a time, and `bash sdlc/scripts/lint`
pass on this candidate. `make test` took 98.84 seconds. `make spec` reports
`306 passed, 6 skipped`, unchanged from the base. Drafts 0092 and 0094 came in
with the candidate. Verify filed draft 0095 for the download cache the `spec`
recipe deletes on every run, which predates this ticket.
