---
---
# The currency scan reads a mention as a build

Ticket 0085 repaired the four defects it named in `tests/built-executable-currency.sh`.
Three residues stand beside them, all measured on 2026-09-11 by reading the
file and running it.

## `build_pattern` matches a mention, not a call

`build_pattern='scripts/require-built-commands\.sh'` at line 50 is a text
match over a code line, and `first_line` takes the lowest matching line. A
harness whose line 2 is `printf 'scripts/require-built-commands.sh\n'` inside
a fixture generator, whose first use of `target/debug` stands at line 3 and
whose real build stands at line 4, passes: the scan reads line 2 as the build,
finds it below the use, and accepts. The ordering rule the ticket restored is
then unenforced for that harness.

`tests/shell-spawn-cache-isolation.sh` already carries three such lines, at
451, 484 and 513. It reaches into `target/debug` nowhere, so nothing is
unheld today. The exposure arrives the moment a harness does both.

`use_pattern='target/debug/[A-Za-z]'` at line 49 has the mirror shape:
`grep -q 'target/debug/pangopup' notes.txt` counts as a use. That direction is
the safe one -- it refuses rather than accepts -- but it means the harness
floor of four can be met by files that never run the executable.

Reading a hand-off rather than a mention means reading what a line does with
the path, which is the argument-shape form ticket 0071 refuses to take. So
this is a decision about what the rule should say, not a pattern to tighten.

## The inspected-something guard cannot fire

`if (( scanned == 0 ))` at line 90 is the shape ticket 0085 set out to remove:
a floor the code above it already guarantees. `scanned` is incremented at line
73, before the `$self` skip at line 74, and `$repository/tests` always holds
this script; the fixture call site writes its files at lines 107 to 122. The
real protection is `harness_floor=4` at line 148, which can fail, so nothing
passes on nothing today. The dead guard should be removed or made live.

## The self-exemption is whole-file

`[[ "$name" != "$self" ]] || continue` at line 74 excuses every line of
`tests/built-executable-currency.sh` from the rule. Only lines 49 and 50, the
pattern definitions, need excusing. This gate is the one harness never held to
its own rule.

## Two files disagree about what a code line is

`tests/built-executable-currency.sh:43` records why `^[^#]*` anchoring is
wrong: it hides a match from every code line carrying an earlier `#`.
`tests/negative-assertion-strength.sh` uses that anchoring at lines 266 and
634 to decide whether a harness sources a support file. The direction is safe
-- it refuses rather than accepts -- but the two files now state opposite
rules about the same thing.

Found during the code review of ticket 0085.
