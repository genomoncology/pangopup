---
---
# A qualification harness fails without saying what failed

`tests/executable-delivery.sh` ends several of its checks in a bare command
under `set -e`. `expect_installer_failure` runs `grep -Fq "$expected"
"$root/rejected.err"` as its last line, so a rejection message that stops
matching stops the harness with status 1 and no output naming the expectation,
the argument, or the line.

Measured in this checkout on 2026-09-10. Changing `install.sh` line 23 from
`unknown argument: $1` to `unrecognised option: $1` made the harness exit 1
after printing two lines, both of them the version banner from earlier work.
Nothing said which assertion failed. A maintainer reads an exit code and
bisects to find the reason.

The same shape appears in the bare `[[ ... ]]` assertions in the same file,
where `set -e` carries the refusal and no message accompanies it, while the
file already has a `fail` helper that prints one.

Ticket 0071 holds its own gate to naming the file and the line of what it
refuses. A qualification harness that refuses a release deserves the same
standard.

Done, observably:

- A failing assertion in `tests/executable-delivery.sh` prints what was
  expected and what was found before the harness exits.
- The harness still fails on every input it fails on today.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes, and the cache home source point stays where it is.
