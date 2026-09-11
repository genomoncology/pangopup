---
flow: build
priority: 1
---
# Three routine actions fail for reasons unrelated to what they test

A gate refuses a tree it should accept, a test refuses a service that shut down
correctly, and `cargo clean` stops part way. None of the three failures says
anything about the software under test. All three were measured in this
checkout on 2026-09-11, and together they mean a green ladder is not evidence
that the ladder passed. That is what this ticket is for: a run of the gates
either proves the software or names a real defect, and never reports a third
thing.

They are one ticket because they are one job. Each was filed separately while
another ticket was in flight, and each survives only because it is rare enough
to rerun past.

## A gate drops a match when grep closes the pipe first

`tests/shell-spawn-cache-isolation.sh` runs under `set -euo pipefail` and asks
its questions as `something | sed | grep -q ... && return 0`. `grep -q` exits on
its first match and closes the pipe, `sed` dies of SIGPIPE with status 141, and
`pipefail` makes 141 the pipeline's status, so the match grep just found is
discarded. Whether it happens depends on how the two processes are scheduled.

Measured over 40 sequential runs on a 16-core machine carrying 24 busy loops:
**7 failures, 17.5 per cent**. Six refused from `runs_an_argument`:

    shell spawn cache isolation: 1 script(s) run an executable handed to them,
    but 2 are named with the gate that reads them, so this scan is checking the
    wrong files

Two refused from `hands_over`, a second site with the same shape. One run
printed both. One run printed a refusal and then a successful summary line, so
a single run can drop a match in one place and keep it in another. The gate
examined the same 53 shell sources in all 40 runs, passing and failing alike.

The silent direction is worse than the loud one. Here the gate refused a tree
it should accept. The same discarded match inside `argument_runners` leaves a
script that runs an executable handed to it out of the counted set, so a script
added tomorrow with that shape would be neither refused nor counted.

## A test refuses a service that shut down correctly

`status_under_threads` in `crates/pangopup-cli/tests/http_service_lifecycle.rs`
starts a real `pangopup serve`, sends `SIGTERM`, and asserts the process exited
successfully. That assertion failed once on 2026-09-11 inside a `make spec` run,
which reported `313 passed, 1 failed` and exited 2.

It has not reproduced: four `make spec` runs gave one failure and three clean,
the named test passed 15 of 15 standalone and 12 of 12 with every core held
busy, and the service fixture suite passed 20 of 20. The failing test and the
service it starts carry the same git blobs as `origin/main`.

Two questions sit inside that one failure, and the assertion cannot separate
them. Whether `pangopup serve` can exit non-zero on a `SIGTERM` it accepted is a
question about shutdown. Whether a reader can tell what happened is a question
about the assertion, which prints no exit status, no signal and none of the
service's standard error. The second is answerable now and makes the first
answerable the next time it happens.

## `cargo clean` stops part way

Ticket 0099 moved the downloaded ONNX Runtime library out of `target/` so that
`cargo clean` stops costing an 87 MB download. `cargo clean` is the routine
command that argument rests on, and in this checkout it exits 101:

    error: failed to remove directory
    `.../target/production-release-qualification-test/data/pangopup/bundles/qualified/bundle`
    Caused by: Permission denied (os error 13)

The directory is mode 555, created that way on purpose by
`tests/production-release-qualification.sh` because a bundle the release cannot
write is part of what the harness qualifies. The harness restores the mode at
the start of its *next* run, so it recovers from its own residue and leaves it
standing for everything else. Clearing that one exposes four more under
`target/spec/gene-naming` and `target/spec/local-assets`, left by the spec
suite. `cargo clean` exits after deleting part of `target/`, so the operator is
left with a half-cleaned build directory and a command that cannot be rerun to
finish. The shape predates 0099.

Settled: a harness that needs an unwritable directory keeps creating one. What
changes is that the residue does not outlive the harness.

Settled: the shutdown question stays open if the evidence does not answer it.
Making the failure legible is required; changing shutdown behaviour is required
only if a measurement shows shutdown is wrong.

Done, observably:

- A gate that asks whether a file matches something returns the same answer
  every time it is asked, measured over hundreds of runs of the shape under
  load rather than argued from the source.
- `tests/shell-spawn-cache-isolation.sh` passes 40 consecutive runs on a loaded
  machine, and still finds both of the scripts that run an executable handed to
  them.
- A pipeline whose exit status is discarded by a downstream reader closing the
  pipe is refused wherever it stands in this repository, and the refusal names
  the file and line.
- A failing service-shutdown assertion prints the exit status, the signal if
  there was one, and the service's own standard error, so a rare failure
  arrives with something to act on.
- `cargo clean` succeeds on a tree where every gate has run, measured by running
  every gate and then the command.
- `make lint`, `make test` and `make spec` pass, and `make test` wall time does
  not grow by more than two seconds.

Boundary: this ticket changes no score, position, status, reason or provenance
field, and adds no route, flag or output. It does not revisit where the ONNX
Runtime library is cached, which ticket 0099 settled, and it does not change
what any gate checks for -- only whether the gate returns the answer it found.
The harnesses keep creating the unwritable directories they need. It publishes
no new number about scoring and it names no private consumer of this software.
