---
flow: build
priority: 2
---
# The gate ladder does not check or report what it claims

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

## The currency scan reads one build line and calls a `grep` argument a build

`tests/built-executable-currency.sh` holds that every harness under `tests/`
reaching into `target/debug` calls `scripts/require-built-commands.sh` before
its first use. Measured on 2026-09-10 while reviewing ticket 0071, it carries
four defects of its own. Its patterns are anchored at `^[^#]*`, so a code line
carrying an earlier `#` never matches -- `trimmed=${bin#$PWD/}; "$repo/target/debug/pangopup" --version`
is ordinary shell that the scan reads as commentary, which is the one direction
this gate must never fail in. It reads the first matching line only, through
`head -n 1`, for both the use and the build, so a build after the first use is
invisible. Its build pattern matches the word `cargo` anywhere in a code line,
and `tests/executable-delivery.sh` carries eight lines passing a cargo command
to `grep` as a string to search for, every one counted as a build. Its floor,
`[[ "$guarded" -ge 2 ]]`, can never fail, because the loop above it already
required two named harnesses to be in the set it counts. Ticket 0071's gate was
written from this file and inherited all four; they were repaired there, and
this is the same repair for the file they came from.

## The inheritance check reads a mention and never reads order

`tests/shell-spawn-cache-isolation.sh` covers `scripts/smoke-linux-release.sh`,
which runs an executable handed to it and names none, by requiring every shell
file naming that script to be a harness the scan accepted. That rule is weaker
than the sentence it stands for, in two directions. It never reads order:
`inheritors_hold` asks whether the caller takes a cache home anywhere in the
file, while `examine` compares line numbers for a named run.
`tests/executable-delivery.sh` hands the smoke script an executable at lines 47
and 71 and takes its cache home at line 83; those two calls pass a stub the
harness writes itself and an explicit `SMOKE_CACHE`, and the one call handing
the real executable is at line 102, after the cache home -- but the check would
read the same either way. It also reads a mention rather than a hand-off, since
the scan is `grep -lF` over whole files, so a path named in a comment is a
caller; the gate exempts itself by name for exactly that reason. Separating a
stub from the shipped executable means reading what each call passes, which is
the argument-shape form ticket 0071 refuses to take, so this repair is not a
matter of tightening a pattern.

## A qualification harness fails without saying what failed

`tests/executable-delivery.sh` ends several checks in a bare command under
`set -e`. `expect_installer_failure` runs `grep -Fq "$expected" "$root/rejected.err"`
as its last line, so a rejection message that stops matching stops the harness
with status 1 and no output naming the expectation, the argument, or the line.
Measured on 2026-09-10: changing `install.sh` line 23 from `unknown argument: $1`
to `unrecognised option: $1` made the harness exit 1 after printing two lines,
both the version banner from earlier work. Nothing said which assertion failed,
so a maintainer reads an exit code and bisects. The same shape appears in the
bare `[[ ... ]]` assertions in the same file, while the file already has a
`fail` helper that prints a message.

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
- A harness whose use line carries an earlier `#` is held by the currency scan;
  a build after the first use is seen; a cargo command line inside a `grep`
  argument is not counted as a build; and every assertion in that file can fail.
- A file that hands an executable to the smoke script before taking a cache home
  is refused, or the inheritance rule states in one sentence what it does hold
  and a check proves that sentence. A file that only mentions the smoke script's
  path in commentary is not read as a caller.
- A failing assertion in `tests/executable-delivery.sh` prints what was expected
  and what was found before the harness exits, and the harness still fails on
  every input it fails on today.
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
