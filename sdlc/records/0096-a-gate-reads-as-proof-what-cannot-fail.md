---
base: be9b79a9e0b2c27776320eaf4715a8eb67c223e0
head: a5c37817d68fb768dca62c469a3328d3b44cabd2
---
# A gate reads as proof what cannot fail

Thirteen negative assertions in `tests/executable-delivery.sh` and
`tests/production-release-qualification.sh` were bare `! grep` or `! cmp` under
`set -euo pipefail`. `set -e` does not exit on an inverted command, so each one
found what it forbade, carried on, and let the gate report success. They are now
22 `refuse_text` calls against `tests/support/forbidden-text.sh`, which matches
the text literally, names the text, the file and what the text means, refuses a
file it cannot read, and exits itself, so a caller that writes nothing after the
call still stops. The two ARM64 `env:` keys in `tests/ci-platform-support.sh` go
through a new `require_workflow_setting`, which reads the workflow the way
ticket 0053's `require_workflow_command` does and refuses a match standing
behind a `#`, with wording that reads correctly over a setting rather than a
command. `tests/negative-assertion-strength.sh` and
`tests/workflow-setting-anchoring.sh` hold both shapes and join
`PORTABLE_QUALIFICATION`.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME`,
`XDG_DATA_HOME` and `TMPDIR`, with real `CARGO_HOME` and `RUSTUP_HOME`, on
throwaway exports of this commit and of the base beside it.

The headline was run as the careless change it is. A job-level `permissions:`
block granting `contents: write` was added to `package-linux.yml`, the way
someone adding a release step writes it. The candidate refused with `forbidden
text stands in a file a gate reads: contents: write` naming the workflow and
`a write permission that would let the packaging workflow publish`. The base,
with the same grant standing, printed `executable delivery tests passed` and
exited 0. A whole `gh release create` step added to the same workflow drew
`forbidden text stands in a file a gate reads: release create` on the
candidate; the base exited 0 on that too.

The glibc ceiling was moved on a copy of `scripts/qualify-linux-release.sh`. A
`2.35` comparison added beside the shipped `2.39` one drew `forbidden text
stands in a file a gate reads: "$maximum" 2.35`, naming the script and `a glibc
ceiling below the one the published executable is built against`; the base
exited 0. Lowering the shipped comparison outright is caught on both, silently
and before the new guard is reached, by the pre-existing positive
`grep -Fq '"$maximum" 2.39'` at `tests/executable-delivery.sh:448`. The new
guard is what catches a `2.35` ceiling reintroduced beside the current one.

Each of the two ARM64 `env:` keys was commented out in `ci.yml` in turn. The
candidate refused both with `workflow setting does not stand in code`, naming
the key, the workflow and the step named `Check the Linux ARM64 command build`.
The base exited 0 on both.

A maintainer doing ordinary work is not obstructed. A new `Show the packaged
inventory` step added to `package-linux.yml`, a new
`! grep ... || fail ...` assertion appended to `tests/executable-delivery.sh`,
and a new `refuse_text` call appended to the same file each left both
`tests/executable-delivery.sh` and `tests/negative-assertion-strength.sh` green;
the new call raised the reintroduction count from 22 to 23 on its own. A bare
`!` appended to that harness was refused by file and line. Verify committed one
in-scope repair: that refusal now names `refuse_text <file> <text>
<description>` and `! cmd || fail ...` as the two shapes that replace it, so a
maintainer reads the repair out of the refusal.

Nothing shipped changed. `.github/workflows/ci.yml`,
`.github/workflows/package-linux.yml`, `scripts/qualify-linux-release.sh`,
`AGENTS.md`, `planning/artifacts/050-public-linux-release.md`,
`planning/artifacts/055-public-v0.3.0.md` and every other path under
`.github/workflows/` and `planning/artifacts/` carry the same git blob at this
commit as at the base.

Four costs are accepted rather than carried as open work.

Three negation shapes still walk past the scan: `!(cmd)` with no space,
`a && ! cmd`, and a one-line function body. Closing them costs 34 false
positives across `install.sh` and the release scripts, every
`[[ -f "$x" && ! -L "$x" ]]` among them. Draft 0098 holds it. `! (cmd)` with a
space is flagged, so only the unspaced form escapes.

`^    runs-on: ubuntu-22[.]04$` widened to the literal `runs-on: ubuntu-22.04`,
which now also refuses `ubuntu-22.04-arm`. Judged correct: that is still a 22.04
image, older than the machine the release is built on, and refusing it is the
rule the gate means. `package-linux.yml` holds one `runs-on`, line 16,
`ubuntu-24.04`.

Two of the 22 forbidden texts point at a file the harness builds while it runs,
so reintroducing them proves the refusal mechanism rather than that the guard
reads what ships. Section 5 of `tests/negative-assertion-strength.sh` writes
both with an empty file, so the exemption is counted and a third one appearing
is a change that list has to be told about.

A `return 1` implementation of `refuse_text` would be indistinguishable from
`exit 1` at every call site, because all 22 calls are bare top-level commands
under `set -e`. Not a defect.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
all seventeen `PORTABLE_QUALIFICATION` gates run one at a time, and
`bash sdlc/scripts/lint` pass on this candidate. `make test` took 104.71 seconds
on a warm second run in a fresh cache root, against a 108.52-second baseline;
the first run in that root rebuilds and took 102.90 seconds. `make spec` reports
`306 passed, 6 skipped`, unchanged from the base.
`tests/negative-assertion-strength.sh` reports 6477 statements in 43 shell
files, 5 negations, 0 bare, and 22 forbidden texts refused when reintroduced.
`tests/workflow-setting-anchoring.sh` reports 2 guarded settings. Draft 0098
came in with the candidate; verify filed none.
