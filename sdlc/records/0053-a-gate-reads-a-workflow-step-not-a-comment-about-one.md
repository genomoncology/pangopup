---
base: 6db594dc2093db490c8aec590ecc44a8e8ff4dd0
head: 22ee7504db1fb3647f4788c190e2295fd51e1a80
---
# A gate reads a workflow step, not a comment about one

`tests/support/workflow-commands.sh` holds one function,
`require_workflow_command`. It strips each line of a workflow at the first `#`
and looks for the command in what is left, so a command standing behind a `#`
is not found. The command is matched as literal text, never as a pattern, and a
third argument narrows the search to the lines of one named step. On refusal it
names the command, the workflow file and the step. Both
`tests/ci-platform-support.sh` and `tests/executable-delivery.sh` source that
file, and thirteen commands between them now go through it: four in
`.github/workflows/ci.yml`, nine in `.github/workflows/package-linux.yml`.

`tests/workflow-command-anchoring.sh` proves it. It reads the guarded commands
out of the two harnesses rather than out of a list of its own, so a command
added to either harness is covered without anyone remembering to add it here,
then comments each one out in turn and requires the harness that guards it to
refuse. It is registered in `PORTABLE_QUALIFICATION` and runs in 0.19 seconds.

`tests/shell-spawn-cache-isolation.sh` was tightened in the same change. It
asked whether a file names `scripts/smoke-linux-release.sh`; it now asks
whether a file runs it, reading a command in an `if` or `while` condition as
the command it is. That was needed because the new gate names the smoke path as
one of the guarded commands while running no executable at all.

Neither workflow changed. `.github/workflows/ci.yml` and
`.github/workflows/package-linux.yml` are byte-identical to the base commit,
sha256 `e7306111f20906c63b5ad0a4f7cc249a6f8e5f20fc9f8e58ac94ac6c029a7bf3` and
`7f6ee56c832fd0e795df74605da7bf6c90af7f90fb80d80d1fdc14e7d0fa2ba5` on both
sides.

The two scenarios the ticket exists for were run whole on a temporary copy. The
ARM64 cross-compile's `run:` line was commented out and `run: "true"` put in its
place. The file still parsed as YAML, and `tests/ci-platform-support.sh`
refused:

    workflow command does not stand in code: run: cargo check --locked --target aarch64-unknown-linux-gnu --package pangopup-cli
      no line of .../.github/workflows/ci.yml runs it outside a comment, in the step named Check the Linux ARM64 command build

The same edit to the release build line in `package-linux.yml` drew the matching
refusal from `tests/executable-delivery.sh`:

    workflow command does not stand in code: cargo build --locked --release --package pangopup-cli
      no line of .../.github/workflows/package-linux.yml runs it outside a comment

The base commit's own copy of each harness was run against each sabotaged file
and exited 0, which is the defect the ticket reported.

Ordinary maintenance is not obstructed. A new step added to `ci.yml` left the
gate passing. Renaming a guarded step refused, and the refusal named the
command, the file and the step name it looked for, which is what a maintainer
needs to fix it. A comment repeating a line that still runs passes; a trailing
comment beside a `run: "true"` refuses.

Three costs are accepted rather than filed again. The two cross-compile `env:`
lines in `ci.yml` stay on the older `require_text`: they are YAML mapping keys,
not commands, and the ARM64 protection is largely not defeatable through them
because `cargo check` does not link and the `cc` crate derives the cross
compiler from the target. Draft 0097 holds that. `tests/shell-spawn-cache-isolation.sh`
deliberately does not read `{` as a command separator, because `{` also opens
`${repo}` and a rule that read it would refuse every
`"${repo}/scripts/smoke-linux-release.sh"` argument; an env-prefixed command and
a path built by concatenation are the two further shapes that leaves open, and
both are judged theoretical. The `CARGO_NET_OFFLINE=true cargo cyclonedx` guard
had no whole-line match before this change, so the call added for it closed a
live hole rather than standing beside a working check.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`,
all fifteen `PORTABLE_QUALIFICATION` gates run one at a time, and `bash
sdlc/scripts/lint` pass on this candidate. `make test` took 108.10 and 111.72
seconds. The same tree with the change reverted took 108.52 seconds on the same
machine in the same session, so the ticket's two-second budget holds and the
spread is the machine. `make spec` reports `306 passed, 6 skipped`, unchanged
from the base. Drafts 0096 and 0097 came in with the candidate. Verify filed
nothing new.
