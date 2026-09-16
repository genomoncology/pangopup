---
flow: build
priority: 5
---
# The published-claim evidence check runs on macOS and Linux

Mac `make test` now reaches `tests/published-claim-evidence.sh` and stops while checking the selected index-size claim. BSD `sed` rejects the one-line looping label used to group digits. The same check's fixture edits use GNU-style in-place `sed`, which BSD `sed` rejects. A syntax failure prevents the check from proving either the clean claim or its negative cases.

The published-claim check must run on both supported operating systems. It must still accept its clean fixture and copy-edited positive fixture, reject each altered claim that lacks measured evidence or correct arithmetic, and report a real changed fixture rather than a mutation that did nothing.

Done, observably:

- `bash tests/published-claim-evidence.sh` exits 0 on macOS and Linux. Its index-size positive and negative fixtures still exercise real edits and produce their intended verdicts.
- The other fixture-mutation refusals in the same check still change their copies and name the unsupported claim. No refusal passes because a mutation failed to run.
- Mac `make test` advances beyond this check without a BSD `sed` syntax error.
- Run `make lint`, `make test`, and `make spec` before commit, and report separate remaining Mac failures honestly.

Boundary: do not change PangoPup's published size figures, score semantics, production index, or the claim rules themselves. Record Mac and Linux results in `sdlc/records/`; no user-facing contract changes.
