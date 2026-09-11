---
---
# A gate drops a match when grep closes the pipe first

`tests/shell-spawn-cache-isolation.sh` runs under `set -euo pipefail` and asks
whether a script runs something it was handed:

    logical_lines "$source" \
        | sed -E 's/\[\[.*\]\]//g' \
        | grep -qE -- "$argument_lead[[:space:]]*\"\\\$\{?$variable\}?\"[[:space:]]" \
        && return 0

`grep -q` exits on its first match and closes the pipe. When `sed` still has
output to write it dies of SIGPIPE with status 141, and under `pipefail` that
141 is the pipeline's status -- so the match grep found is thrown away. Whether
it happens depends on how the two processes are scheduled.

Measured on 2026-09-11 in this checkout. The same pipeline shape, with a
pattern that matches the first line, returned 141 in 20 of 40 runs and 0 in the
other 20. The gate itself passed 12 of 12 standalone runs, and refused once
inside a `make test` run on a loaded machine with:

    shell spawn cache isolation: 1 script(s) run an executable handed to them,
    but 2 are named with the gate that reads them, so this scan is checking the
    wrong files

It had scanned the same 53 shell sources as every passing run. The dropped
match is `pangopup` in `scripts/run-production-qualification.sh`;
`runs_an_argument` then tries the remaining variables, finds none, and reports
that the script runs nothing it was handed.

Both directions are wrong, and the silent one is worse. Here the gate refused a
tree it should accept. The same discarded match in `argument_runners` leaves a
script that runs an executable handed to it out of the counted set, and a
script added tomorrow with the same shape would be neither refused nor counted.

`hands_over` has the same shape, and so does every other
`something | grep -q ... && ...` under `pipefail` in this file.

What a successor must prove: the answer does not depend on scheduling -- run
the shape a few hundred times and see one answer -- and the two scripts that
run an executable handed to them are still both found.

## The rate, measured

Verify ran the gate 40 times in sequence on this candidate, on a 16-core
machine carrying 24 busy loops, on 2026-09-11. It failed 7 times, 17.5 per
cent.

Six of the seven refused with the message above:

    shell spawn cache isolation: 1 script(s) run an executable handed to them,
    but 2 are named with the gate that reads them, so this scan is checking the
    wrong files

Two of the seven printed the `hands_over` refusal instead, which confirms the
prediction at the end of this draft that the same shape stands there too:

    examined /home/ian/workspace/repos/pangopup and found no shell file running
    scripts/smoke-linux-release.sh, so the inheritance rule held over nothing

One run printed both. One run printed the `hands_over` refusal and then the
successful second summary line, so a single gate run can drop a match in one
place and keep it in another.

The gate examined 53 shell sources in every one of the 40 runs, passing and
failing alike, so the file set does not vary. A successor repairing this has
17.5 per cent as the number to drive to zero, and 40 runs under load as the
shape of the measurement.
