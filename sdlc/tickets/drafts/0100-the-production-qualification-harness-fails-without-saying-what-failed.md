---
---
# The production qualification harness fails without saying what failed

Ticket 0085 repaired `tests/executable-delivery.sh`: every assertion in it now goes through `tests/support/expected-text.sh` and names what it wanted, where it looked and what stood there instead, and `tests/negative-assertion-strength.sh` holds that file against the bare shape coming back.

`tests/production-release-qualification.sh` carries the same defect and was left out of that repair. Measured on 2026-09-11: 69 statements in it are led by `[[` or `grep` with nothing consuming their exit. Under `set -euo pipefail` each one stops the harness with status 1 and no output, so a maintainer reads an exit code and bisects. Among them:

- `grep -Fxq 'production qualification passed' "$root/$label.out"` at line 267.
- `[[ $(grep -Fc $'snv\t' "$root/lookups.log") == 7 ]]` at line 270, and two more counts beside it.
- twenty-odd `grep -Fxq '<a refusal message>' "$root/<case>.err"` lines, each pinning the exact text one refusal prints.
- `[[ $HOME == "$QUALIFICATION_EXPECTED_HOME" ]]` and the two cache-home comparisons at lines 49 to 51.

The repair is the one already written: `require_text`, `require_line`, `require_pattern` and `equal` from `tests/support/expected-text.sh`, and the harness added to `assertion_harnesses` in `tests/negative-assertion-strength.sh`, which is named there as the one file the rule does not yet cover.

It was left out because it cannot be exercised the way `tests/executable-delivery.sh` was. It drives a real production release against an operator's own installed bundles, so a rewrite of 69 assertions cannot be proved green in the checkout the way the other harness could. Whoever takes this needs a machine that can run it.

Found during the design stage of ticket 0085.
