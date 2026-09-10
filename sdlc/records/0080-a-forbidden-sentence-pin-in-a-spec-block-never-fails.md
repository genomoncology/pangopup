---
base: 0529c287db58b86945fa388e71ce2f05f9051250
head: 2c5b7bceadfcf81dd488cfba92118df95183d6ac
---
# A forbidden-sentence pin in a spec block never fails

A spec block refuted a claim with a leading `!`. `set -e` ignores a pipeline
that begins with that reserved word and mustmatch reports the status of a
block's last command, so every one of those lines checked nothing. The form is
gone from `spec/` entirely. Refutations now go through
`scripts/spec-refutes.sh`, which exits non-zero when the claim breaks and stops
the block on the line that broke. `--absent` refutes a pattern in a file or in
piped text; `--fails` refutes a command that must not succeed.

The helper also refuses a refutation that proved nothing: an empty file, empty
piped text, a path it cannot read, a command that was never there to run, a
call naming no pattern or no command, and a call that names no path while
standard input is a terminal. That last one would otherwise wait forever, and a
gate has to fail rather than wedge.

`tests/spec-refutation-evidence.sh` keeps the hole shut. It refuses any spec
line starting with `!`, requires every block carrying a refutation to call
`mustmatch` so the block runs at all, requires every refutation that is not fed
by a pipe to name a path, refuses `mkfifo` in a spec block, and holds the
population to a floor of 27 rather than to a number with slack in it.

One pin is removed rather than converted, and that is accepted rather than
carried as open work. `spec/runtime-transport.md` claimed that a transport
whose member is a FIFO is refused. `pangopup-build runtime-transport verify`
opens every member, a FIFO member has no writer, and the open never returns, so
a live pin on it stops the gate instead of failing it. The pin and the three
lines that built the FIFO transport are gone, the reason sits in the spec file
beside them, and the FIFO check keeps the fixture from coming back while verify
still hangs on it. The product defect is carried beside the ticket as a draft.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME` and
`TMPDIR`. The ticket's own measurement was reproduced: appending `Store this
identity as the data-set version` to `README.md` now fails the `What a consumer
pins` block, naming the sentence it found, where before it passed. Each shape
of the inert form was reintroduced and refused -- a `!`-led line at column
zero, one indented inside a `for` loop, and a correct refutation moved into a
block that calls no `mustmatch`. Deleting one pin was refused by the floor at
26 against 27. Restoring `mkfifo` to a spec block was refused with the reason.
Dropping a path argument from a refutation was refused before it could reach a
gate, and the helper on a real terminal refused in milliseconds rather than
waiting. The gate's reported counts were checked against the tree: 24 files
under `spec/`, 27 helper calls in them.

The block at `spec/runtime-transport.md:79` used to be skipped whole. The base
file under the same mustmatch reports `10 passed, 1 skipped`; this one reports
`11 passed`. Its three claims were neutralised one at a time -- the corrupt
member left intact, the substituted NOTICE left unwritten, the symlinked member
left a faithful copy -- and each time the block failed on the line whose claim
had gone. A required pin in the same subsystem still bites when its sentence
leaves `README.md`.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
`bash tests/spec-refutation-evidence.sh` and `bash sdlc/scripts/lint` all pass
on this candidate. `make spec` reports `304 passed, 6 skipped`; the six blocks
that still never execute are carried as a draft. No document changed its claim,
and no required pin was weakened to make a refutation work.
