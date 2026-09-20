---
---
# A pipeline into `head` loses the producer's status the same way

`tests/pipeline-match-integrity.sh` refuses a pipeline feeding a quiet `grep`
and deliberately leaves `head` alone. Its reason is sound as far as it goes:
`head` exits 0 whether it read anything or not, so no answer is carried in its
status and no answer can be lost.

What is still lost is the producer's status. Under `set -euo pipefail`,
`x=$(producer | head -n 1)` where `producer` has more to write than a pipe
buffer holds exits 141, and `set -e` ends the script -- for a reason that says
nothing about what the script was checking. That is the loud half of the same
defect ticket 0113 opened on, and eighteen of them stand in the tree.

Two of them sit inside the gates 0113 is about:

    tests/shell-spawn-cache-isolation.sh:163   code_lines "$1" "$2" | head -n 1
    tests/built-executable-currency.sh:78      code_lines "$1" "$2" | head -n 1

`code_lines` is itself a three-stage pipeline over a whole file, and
`first_line` is called from a `$( )` under `set -euo pipefail`. Today the
output is a handful of line numbers and fits the pipe buffer, so `cut` finishes
before `head` closes it. A file with enough matching lines closes that gap.

The others stand in `scripts/qualify-container.sh`,
`tests/published-claim-evidence.sh`, `tests/route-disagreement-rate.sh`,
`tests/model-cache-layout-history.sh`, `tests/negative-assertion-strength.sh`,
`tests/harness-rule-coverage.sh`, `scripts/spec-refutes.sh` and
`tests/release-help-contract.sh`.

Other readers close a pipe early the same way: `sed q`, `awk '{exit}'`,
`grep -m1`, `grep -l`, and a bare `read`. None stands in a dangerous position
in the tree today.

Done, observably: a pipeline whose producer's status is discarded by a reader
that stops early is refused wherever it stands, the refusal names the file and
the line, and a measurement shows the loss happening rather than arguing it
from the source.
