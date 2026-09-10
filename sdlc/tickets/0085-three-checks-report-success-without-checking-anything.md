---
flow: build
priority: 2
---
# Three checks report success without checking anything

Three separate places in the gate ladder report success while checking nothing.
They are one ticket because they are one failure: a green result that means only
that nothing looked. Ticket 0080 closed this shape for the refutation form; these
are what it left behind and what its own review turned up.

## A spec bash block that asserts nothing is skipped whole

mustmatch runs a plain bash block only when the block calls a `mustmatch`
command. A block that ends without one is reported `SKIP` and none of its lines
run. The summary counts it as a skip beside the passes, so nothing in a gate log
says which claim went unchecked.

Measured in this checkout on 2026-09-10, during ticket 0080. Ticket 0080 took
`spec/runtime-transport.md:79`, the block holding the transport-tamper claims,
which had never executed a line. Six remain:

- `spec/container-image.md:9` runs `bash tests/container-delivery.sh`.
- `spec/model-kernel.md:7` runs `pangopup-build model inspect`.
- `spec/model-kernel.md:20` runs `pangopup-build model qualify`.
- `spec/runtime-profile.md:45` checks that no profile was written.
- `spec/runtime-release.md:58` checks that no release directory was created.
- `spec/runtime-release.md:77` checks that no staging directory was left behind.

The two `model-kernel.md` blocks are each followed by a text block with no
`expect=` attribute, so the transcript beneath them pins nothing either.

## A `--fails` refutation holds on a command that could not be run

`scripts/spec-refutes.sh --fails <command...>` refuses status 127, because a
command that was never there proves nothing by failing. A command that is there
and cannot be run exits 126, and 126 falls through to the branch reading any
other non-zero status as the refutation holding.

Measured in this checkout on 2026-09-10, during the code review of ticket 0080.
A shell script with mode 644 was named to `--fails`: the helper exited 0 and
reported nothing, so the pin was green on a command that never ran a line. The
same status comes back when the named path is a directory.
`spec/runtime-transport.md` names `pangopup-build` on three `--fails` pins, so a
build leaving the executable without its execute bit turns all three green.

## Nothing catches a hand-wrapped string literal that lost its continuation

A long Rust string literal here is wrapped by hand: a `\` ends the line and the
next is indented under the opening quote, so the runtime text reads as one
sentence. When the `\` is missing the wrap collapses and the indentation
survives as a run of internal spaces inside the literal. It compiles, asserts
the same thing, and reads wrong the moment anyone sees it. Five assertion
messages in `crates/pangopup-cache/src/lib.rs` arrived this way and were
repaired under ticket 0075, caught by eye. No gate saw them: `cargo fmt` does
not reflow literal contents, clippy has no lint for it, and `sdlc/scripts/lint`
adds nothing that would. The pattern is mechanical -- three or more spaces
between two word characters inside a Rust source file -- and the whole crate
tree contains it in exactly one deliberate place, the aligned usage text in
`crates/pangopup-build/src/main.rs`.

Done, observably:

- A spec bash block that mustmatch would skip fails a gate, naming the block.
  Every block listed above either asserts something or is removed with a reason.
- A `--fails` pin whose command could not be run fails, naming the command, and
  `tests/spec-refutation-evidence.sh` holds that against a fixture beside the
  127 case it already holds.
- A collapsed string literal fails the lint ladder, with the one deliberate
  alignment case exempted by an exemption no wider than that case.
- Each of the three checks counts what it inspected, so a scan matching nothing
  fails rather than passes.
- `make test`, `make spec` and `make lint` pass.

Boundary: this states no new claim about what the software does. It makes checks
that already exist actually check, and adds one lint rule. It changes no product
source under `crates/` except a string literal a check refuses, alters no score,
position, status, reason or provenance field, and adds no route, flag or output.
It does not fix the FIFO defect that ticket 0082 carries, and it must not
restore the FIFO pin ticket 0080 removed with a reason. 127 keeps the message it
has. `spec/container-image.md:9` invokes a container harness; whether that
harness belongs in `make spec` is part of the decision, not a foregone one. It
names no private consumer of this software.
