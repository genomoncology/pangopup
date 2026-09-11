---
---
# The currency scan reads one build line and calls a `grep` argument a build

`tests/built-executable-currency.sh` holds that every harness under `tests/`
reaching into `target/debug` calls `scripts/require-built-commands.sh` before
its first use. Measured in this checkout on 2026-09-10 while reviewing the
design of ticket 0071, the scan carries three defects of its own.

Its patterns are anchored at `^[^#]*`, so a code line carrying an earlier `#`
never matches. `trimmed=${bin#$PWD/}; "$repo/target/debug/pangopup" --version`
is ordinary shell and the scan reads it as commentary, which is the one
direction this gate must never fail in.

It reads the first matching line only, through `head -n 1`, for both the use
and the build. A harness whose first `target/debug` line is late and whose
build is later still is not seen, and neither is a second use before the build.

Its build pattern matches the word `cargo` anywhere in a code line.
`tests/executable-delivery.sh` carries eight lines that pass a cargo command
line to `grep` as a string to search for, and the scan counts every one of
them as a build.

The floor under the scan, `[[ "$guarded" -ge 2 ]]`, can never fail: the loop
above it has already required two named harnesses to be in the set it counts.

Ticket 0071's gate, `tests/shell-spawn-cache-isolation.sh`, was written from
this file and inherited all four. They were repaired there. This draft is the
same repair for the file they came from.

Done, observably:

- A harness whose use line carries an earlier `#` is held.
- A build after the first use is seen, not just the first build line.
- A cargo command line inside a `grep` argument is not counted as a build.
- Every assertion in the file can fail.

Boundary: no harness behaviour changes and no product source is touched.
