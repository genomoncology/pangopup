---
---
# A build script named in a heredoc body is still read as a build

Ticket 0099 narrowed `build_pattern` in `tests/built-executable-currency.sh` so
that a mention of `scripts/require-built-commands.sh` is not read as a call of
it. The pattern it landed:

    ^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=)?(\$\()?((bash|sh|source|\.)[[:space:]]+)?"?[^"[:space:]]*scripts/require-built-commands\.sh

The comment beside it says each shape it accepts "hands somebody a build". Two
of them hand nobody one. Measured on 2026-09-11 against the shipped pattern:

- A line inside a heredoc body spelling the path on its own --
  `cat <<EOT` ... `scripts/require-built-commands.sh` ... `EOT` -- matches as a
  command word. A harness generating a fixture that way, using the built
  executable below it and building nowhere, is accepted.
- `builder=scripts/require-built-commands.sh` matches whether or not the
  variable is ever run. The assignment shape is deliberate and needed --
  `tests/executable-delivery.sh` spells its helper that way -- but the pattern
  asks only that the assignment stand, never that anything run it.

The `printf` shape ticket 0099 was written about is closed and held by
`tests/harness-rule-coverage.sh`. These two are the same defect in shapes the
line-wise pattern cannot see, because telling a heredoc body from code, or an
assignment that is run from one that is not, needs more than one line of
context.

Nothing in the repository spells either one today. `tests/built-executable-currency.sh`
reports four harnesses reaching into `target/debug`, each with a real call
above its first use.

What a successor must prove: a harness that names the build script only inside
a heredoc body, and one that assigns it to a variable it never runs, are each
refused for building nothing, and the four harnesses that build today are still
accepted.
