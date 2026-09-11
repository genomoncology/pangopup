---
---
# Three negation shapes walk past the bare-negation scan

Ticket 0096 gave `tests/negative-assertion-strength.sh` a scan that reads every shell file in the repository and refuses a statement led by a bare `!` whose exit nothing consumes. That is the shape the thirteen silent assertions were written in, and no shell file carries it any more. The scan is deliberately narrow: a statement counts only when its first non-blank characters are `!` followed by whitespace, and it is exempt only when a `||` stands on the same line outside quotes.

Three other spellings of the same defect are outside what it reads. Each was run on 2026-09-11 under `set -euo pipefail` against a file that does hold the forbidden text, and each printed the line after it and exited 0.

```
!(grep -Fq contents w.yml)
echo REACHED_D            # prints, exit 0

true && ! grep -Fq contents w.yml
echo REACHED_B            # prints, exit 0

check() { ! grep -Fq contents w.yml; echo INSIDE_AFTER_NEGATION; }
check
echo REACHED_E            # both print, exit 0
```

`!(cmd)` has no whitespace after the `!`, so the scan reads it as a word. `a && ! cmd` does not lead its line. The one-line function body carries the negation behind a `{`, and the statements after it inside the body run against what the negation forbids. The scan flags none of the three.

The design review of 0096 measured the cost of closing them: reading a `!` anywhere on a statement rather than only at its head refuses 34 lines across `install.sh` and the release scripts that are legitimate `[[ ! -e ]]`, `if ! cmd`, `while ! cmd` and awk `!hit` forms. A closure therefore needs a real shell parse, or a rule that reads the statement's structure rather than its first two characters, not a wider regex. That work is a ticket of its own.

No shell file in the repository holds any of the three today, measured on 2026-09-11. What is missing is the mechanism that would stop one arriving.

Found during code review of ticket 0096.
