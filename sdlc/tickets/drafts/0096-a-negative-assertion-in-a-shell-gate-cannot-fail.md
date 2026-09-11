---
---
# A negative assertion in a shell gate cannot fail

`set -e` does not exit on a command whose status is inverted with `!`. Thirteen assertions in `tests/executable-delivery.sh` and `tests/production-release-qualification.sh` are written as a bare `! grep ...` or `! cmp ...` under `set -euo pipefail`, so when the thing they forbid is present the command returns 1, the shell carries on, and the gate reports nothing.

Measured on 2026-09-10 in this checkout:

```
set -euo pipefail
! grep -Eq 'contents: read' .github/workflows/package-linux.yml
echo REACHED
```

prints `REACHED` and exits 0, although `contents: read` is there.

The sites are `tests/executable-delivery.sh` lines 66, 290, 408, 449, 450, 451, 452, 453, 454 and 468, and three in `tests/production-release-qualification.sh`. They forbid, among other things, `contents: write`, `attest`, `release create` and `release upload` in the packaging workflow's permissions, a live oracle variant in the workflow, `make lint`, `make test` or `make spec` in the packaging workflow, `ubuntu-22.04` as the runner, and a glibc maximum of 2.35. Every one of those can be reintroduced today without the gate noticing.

`! cmd || return 1` and `! cmd || fail ...` are unaffected; the `||` supplies the exit. Line 605 of `tests/executable-delivery.sh` is that shape and works.

The fix is to give each bare `!` assertion an explicit failure, for example `! grep -Eq ... "$file" || fail '...'`. A gate that enumerates the sites and refuses a bare `!` statement in a `set -e` shell harness would keep the shape from coming back.

Found during code review of ticket 0053, which does not cover it.
