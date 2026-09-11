---
base: 1a7989bdd0591ba43940c2406979c545a1065163
head: ea1b461b11e35663d2bd2fe14a385e5d76553fdd
---
# Three routine actions fail for reasons unrelated to what they test

Thirty-nine pipelines that fed a quiet `grep` under `pipefail` now read their
input with a here-string or a process substitution, and five whose producer's
own status had to hold read that output into a variable first.
`tests/pipeline-match-integrity.sh` refuses the shape wherever it stands and
counts the questions asked, so a question can be rewritten and not deleted.
`tests/shell-matching-determinism.sh` asks the two questions
`tests/shell-spawn-cache-isolation.sh` was losing, 128 times each across
sixteen concurrent askers. `support::assert_shutdown_succeeded` reports the
exit status, the signal and the service's standard error at the fifteen sites
that asserted success and printed nothing. The release qualification harness
and the two spec files that publish a bundle at mode 0555 take the mode back
before they finish, and `tests/build-directory-residue.sh` reads the real build
directory for anything left unwritable.

Exercised beyond the suite with `CARGO_HOME` and `RUSTUP_HOME` at the
operator's own, the four `PANGOPUP_*` names unset, and `ORT_CACHE_DIR` never
exported.

## `cargo clean`, measured end to end

The ticket's central bullet asks for the command itself on a tree every gate
has run over. Earlier stages proved the rule against fixture trees. This is the
command.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
`bash sdlc/scripts/lint` and all 29 `PORTABLE_QUALIFICATION` and 3
`SHELL_QUALIFICATION` gates ran first, the last three of them twice. The build
directory then held 1,366 directories and **0** of them were unwritable.

`cargo clean` exited **0** in 1.15 seconds, removed 18,836 files and 10.5 GiB,
and left no `target/` at all. It finished. On this same checkout the command
exited 101 on a mode-555 directory before this ticket.

`make spec` then ran cold: 146.90 seconds, `314 passed`, exit 0. The inventory
of `$HOME/.cache/ort.pyke.io` taken before the build and after it is identical
-- the same 11 entries, the same sizes, the same modification times to the
nanosecond, the same md5 for all three `libonnxruntime.a` files, and 288,252,072
bytes both times. It is also identical to the inventory taken at the start of
this session, before any gate ran. Zero bytes were written there, nothing was
removed and nothing was rewritten. `ort-sys` emitted one
`cargo:rustc-link-search` into
`/home/ian/.cache/ort.pyke.io/dfbin/x86_64-unknown-linux-gnu/acc1cba7...` and no
`downloading from` line. `target/ort-cache` was not recreated. Ticket 0099's
measurement still holds after this ticket's changes.

The machine received 90,967,690 bytes across the clean and the cold rebuild.
It is shared and two other builds were running throughout, so that figure is
not attributable to this build; the byte-identical cache and the absent
download line are what say no library was fetched.

After the cold `make spec` the build directory held 726 directories, again 0 of
them unwritable, and after the final `make test` 897, again 0.

## The flake, measured a third time

Twenty-four busy loops on sixteen cores, load average 31 at the peak.

`tests/shell-spawn-cache-isolation.sh` on this candidate: **0 failures in 40
runs**. The base version of that one file, copied into the same working tree
under the same load: **11 failures in 20 runs**. Every base refusal was the
same line the ticket names:

    shell spawn cache isolation: 1 script(s) run an executable handed to them,
    but 2 are named with the gate that reads them, so this scan is checking the
    wrong files

The candidate gate reported 56 shell sources and both scripts that run an
executable handed to them on all 40 runs. Design measured 7 in 40 on the base,
code 0 in 40, code review 0 in 40 against a base control of 17 in 20. Three
hands, three loads, the same answer.

## A real shutdown failure, read as an operator

A genuine failure was forced against a real `pangopup serve`: the SIGTERM in
`status_under_threads` was changed to SIGKILL and the fixture test run. The
report read:

    service exit: the service ended with signal 9; its standard error was:

and stopped there. The signal is what a reader could not get before, and it
arrives. What follows the colon does not. `pangopup serve` writes its listening
line to standard output and writes to standard error only when it has something
to report, so the ordinary shutdown failure carries nothing there and the report
ends on a colon. A reader has to decide whether the service was silent or the
capture failed, and neither reading tells them whether the service was starting,
draining or already stopped.

Verify repaired the wording. A failure with nothing on standard error now reads
`the service ended with signal 9 and wrote nothing to its standard error`, and a
failure that carries output keeps it under a sentence of its own.
`a_silent_service_is_reported_as_having_written_nothing` holds both halves. The
exit status, the signal and the captured text are unchanged, and the forced
failure was read again after the repair. That is the whole of verify's change.

The label still does not say which phase the service was in. Nothing in
`pangopup serve` publishes one, and the ticket forbids changing shutdown
behaviour, so the report carries what the process left behind. It now says so
plainly instead of trailing off.

## The rewritten sites

Thirty-nine sites in ten files were read. The `< <(...)` and here-string forms
read as this codebase's own: `grep -qE -- "$smoke_call" < <(logical_lines "$1")`
puts the question first and its input second, which is the order the function
names already read in. The nested form in `tests/route-disagreement-rate.sh`,
`grep -qi 'value' < <(grep -F -- "$figure" <<<"$sentences")`, reads inside out
and is the one place the rewrite costs something; it is still one statement and
still names both halves.

Both directions were checked for a status the repair could drop. A process
substitution discards its producer's status, and the old pipeline under
`pipefail` did not. At every one of these sites the producer is `printf`, `cut`,
`sed` or the file-reading `logical_lines`, and the answer is the same either
way: where the producer emitted nothing, the old pipeline reported the inner
grep's 1 and the new form reports the outer grep's 1. The five sites whose
producer is a command whose failure must stop the script -- four in
`scripts/smoke-linux-release.sh`, three in `scripts/qualify-container.sh` --
read the output into a variable instead, which fails the way the pipeline did.
The comments at both files say why the capture is there and are correct as
written.

## The harness still qualifies what it qualifies

`tests/production-release-qualification.sh` was watched while it ran. It made
**8** directories at mode 0555 during the run -- the qualified bundle and its
wrapper, two more for the multiple-bundle case, and one each for the symlink and
unsafe cases -- and left **0** standing afterwards. The mode restoration is a
trap, so it fires however the harness ends.

The refusal still fires. The stub was changed to publish the installed bundle at
0755 instead of 0555, which is the release writing where it should not. The
harness exited 1 with `installed SNV bundle is unsafe`. The file was copied back
and the tree confirmed clean.

The spec-side recovery was exercised too. Four mode-555 directories were planted
under `target/spec/gene-naming` and `target/spec/local-assets`, standing in for
a run killed before its closing `chmod`.
`tests/build-directory-residue.sh` named all four and refused. `make spec` then
ran, recovered every one through the `chmod` at the head of each spec file,
reported `314 passed`, and left the gate green.

## The ladder

All on this candidate, on a sixteen-core machine shared with two other builds.

`make lint` 2.07 s, exit 0. `make test` 130.77 s warm at load 4.8, exit 0, and
201.11 s on the cold run straight after `cargo clean` at load 9.9 to 14.5.
`make spec` 15.89 s warm and 146.90 s cold, `314 passed` both times.
`scripts/run-service-fixture-tests.sh` 1.80 s, 20 tests matched.
`bash sdlc/scripts/lint` exit 0. Every `PORTABLE_QUALIFICATION` and
`SHELL_QUALIFICATION` gate was also run one at a time, all 32 exit 0.

`make test` wall time is not comparable run to run on this machine: the same
recipe took 130.77 s and 201.11 s within the hour, and
`tests/recipe-spawn-cache-isolation.sh` alone moved from 21.43 s in ticket 0099
to 24.18 s here, untouched by this ticket. What this ticket adds is measurable
directly. The three new gates cost **1.33 seconds** between them:
`tests/build-directory-residue.sh` 0.10 s,
`tests/pipeline-match-integrity.sh` 0.17 s and
`tests/shell-matching-determinism.sh` 1.06 s.
`crates/pangopup-cli/tests/service_shutdown_report.rs` reports its four tests in
0.00 s. That is inside the two seconds the ticket budgets.

Reported counts: `tests/pipeline-match-integrity.sh` 56 shell files, 270
pipeline stages, 289 quiet grep questions, none through a pipeline;
`tests/shell-matching-determinism.sh` 7 questions at 128 asks each across 16
askers, every answer the same; `tests/shell-spawn-cache-isolation.sh` 56 shell
sources, 5 harnesses holding a cache home, 2 scripts running an executable
handed to them; `tests/build-directory-residue.sh` 897 directories, none
unwritable; `tests/negative-assertion-strength.sh` 9,179 statements in 56 shell
files, 0 bare.

The operator's `~/.cache/pangopup/model-results.sqlite3` is byte-identical after
all of it: md5 `0ce402e99f91b4d16ed3aaddc88160fc`, 303,104 bytes, mtime
unchanged. No `pangopup` or `pangopup-build` process is left running.

Verify filed no draft. Drafts 0114 and 0115 came in with the candidate.
