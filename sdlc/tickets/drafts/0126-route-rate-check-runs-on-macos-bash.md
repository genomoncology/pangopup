---
flow: build
priority: 5
---
# The route-disagreement evidence check runs on macOS Bash

Mac `make test` now passes `tests/repository-sourcing.sh` and stops in `tests/route-disagreement-rate.sh` because Bash 3.2 rejects `declare -A`. This check uses associative maps for measurement fields and independently recomputed record counts. The failure prevents the check from proving that the published rate follows from the committed variant set and per-record result.

The check must run under the system Bash on macOS and Linux Bash. It must still read every required measurement field, recompute counts from the per-record file, match the manifest population, verify the stated percentages and limits, and refuse each altered fixture for the same unsupported claim. File content must never become shell code.

Done, observably:

- `/bin/bash tests/route-disagreement-rate.sh` exits 0 on macOS and Linux with the existing measured totals and positive fixture behavior intact.
- Its negative fixtures still change their copied inputs and reach their intended refusal messages; missing fields, wrong counts, wrong population, and unsupported published prose do not pass.
- The copied-input mutation and fingerprint helpers work on macOS and Linux. They do not rely on GNU `sed -i`; every existing negative case still makes a real edit before the check refuses it.
- A hostile copied measurement field or per-record value containing shell syntax cannot run a command or create a marker file. The check refuses the malformed value with a claim-specific message. Keep the real 2,615-variant and 2,790-record population checks intact.
- Mac `make test` advances beyond this check. Run `make lint`, `make test`, and `make spec` before commit and report separate remaining failures honestly.

Boundary: do not change the recorded scores, manifest, measurement percentages, score precision, or production routing. Record Mac and Linux evidence in `sdlc/records/`; no public score contract changes.
