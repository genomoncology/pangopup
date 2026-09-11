---
flow: build
priority: 1
---
# A gate reads as proof what cannot fail

Two shapes in the gate ladder report success over an assertion that cannot go
red. Both were found during the code review of ticket 0053, which closed the
same family for workflow commands and does not cover these. They are one ticket
because they are one job: a gate is reading something as proof when it is not
proof, and the repair in both cases is to make the check able to fail and then
hold that shape mechanically. Both measured in this checkout on 2026-09-10.

## Thirteen negative assertions carry on when they find what they forbid

`set -e` does not exit on a command whose status is inverted with `!`. Thirteen
assertions in `tests/executable-delivery.sh` and
`tests/production-release-qualification.sh` are written as a bare `! grep ...`
or `! cmp ...` under `set -euo pipefail`. When the thing they forbid is present
the command returns 1, the shell carries on, and the gate reports nothing.

```
set -euo pipefail
! grep -Eq 'contents: read' .github/workflows/package-linux.yml
echo REACHED
```

prints `REACHED` and exits 0, although `contents: read` is there.

The sites are `tests/executable-delivery.sh` lines 66, 290, 408, 449, 450, 451,
452, 453, 454 and 468, and three in
`tests/production-release-qualification.sh`. What they forbid is not
incidental. Among it: `contents: write`, `attest`, `release create` and
`release upload` in the packaging workflow's permissions, which decide whether
that workflow can publish; a live oracle variant in the workflow; `make lint`,
`make test` or `make spec` in the packaging workflow; `ubuntu-22.04` as the
runner; and a glibc maximum of 2.35, which is what makes the published Linux
executable run on systems older than the machine that built it. Every one of
those can be reintroduced today with the gate silent.

`! cmd || return 1` and `! cmd || fail ...` are unaffected, because the `||`
supplies the exit. `tests/executable-delivery.sh:605` is that shape and works.

## Two ARM64 compiler settings are still satisfied by a comment

Ticket 0053 named six lines of `tests/ci-platform-support.sh` that proved a
workflow step exists by searching for its text. Five now go through
`require_workflow_command`, which refuses a match standing behind a `#`. Two do
not: the `env:` keys of the ARM64 cross-compile step in `.github/workflows/ci.yml`.

```
require_text 'CC_aarch64_unknown_linux_gnu: aarch64-linux-gnu-gcc' "$arm_step" 'bundled C dependency compiler'
require_text 'CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER: aarch64-linux-gnu-gcc' "$arm_step" 'Rust target linker'
```

`require_text` is a `case` substring test over a joined slice, so commenting out
either line leaves the gate green. The exposure is smaller than the one 0053
closed: `cargo check` does not link, and the `cc` crate derives
`aarch64-linux-gnu-gcc` from the target on its own, so removing the two keys
most likely leaves the cross-compile working or failing loudly rather than
passing on nothing. What remains is a gate that still reads a comment as proof,
in the one file where five neighbours no longer do.

Done, observably:

- Every negative assertion in a shell gate fails when the thing it forbids is
  present. A test proves it by reintroducing each forbidden thing in turn, on a
  copy, and observing the refusal.
- Each refusal names what was found and the file it was found in, so a reader
  knows what to remove.
- A bare `!`-led statement whose exit nothing consumes is refused wherever it
  appears in a shell gate, so the shape cannot come back. The check counts the
  statements it read and refuses a count of zero.
- The two ARM64 `env:` keys are held by a check that refuses a match standing
  behind a `#`, and its refusal reads correctly over a setting rather than
  describing it as a command.
- Every forbidden thing these gates name today is still forbidden, and no gate
  starts passing on something it refused before.
- No shipped workflow changes, and no permission, runner image or glibc ceiling
  changes. This ticket makes existing rules enforceable; it does not rewrite
  them.
- `make lint`, `make test` and `make spec` pass, and `make test` wall time does
  not grow by more than two seconds.

Boundary: this ticket changes how gates read, not what they require. It changes
no product source under `crates/`, alters no score, position, status, reason or
provenance field, and adds no route, flag or output. It does not change
`.github/workflows/ci.yml` or `.github/workflows/package-linux.yml`. It does not
change what the executable prints, the oracles under `tests/fixtures/`, their
pinned digests, or the runbooks under `sdlc/planning/artifacts/`. It does not
reopen the thirteen commands ticket 0053 anchored, and it does not revisit the
three discovery patterns ticket 0050 anchored in
`tests/built-executable-currency.sh`.

It names no private consumer of this software.
