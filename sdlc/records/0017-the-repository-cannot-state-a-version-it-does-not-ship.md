---
base: 0d1d9e883035d8142bd8af1aca96db079deb85b1
head: 639adbce657e07f03c2ed41bd0855e6cd9a326b2
---

# Prepare one coherent 0.4.0 application candidate

The workspace, all eight package lockfile entries, built CLI, HTTP status tests, production qualification checks, container staging workflow, and service architecture now identify application candidate 0.4.0. Public installation, citation, and release material continues to identify released version 0.3.0.

A normal lint gate now classifies candidate claims, current-public claims, historical records, and fixed fixtures. Mutation tests prove that drift in each category fails with the file and category named. The gate parses named PangoPup package versions and does not reject unrelated dependencies that use version 0.3.0.

The active scoring identity changed because the existing identity preimage includes the application version. The identity algorithm, production scoring code, assets, request and response shapes, and fixed identity fixtures did not change.

Independent code review accepted candidate `a4d0b466648fa36d6c50fdf612350133dd52f08c` with no findings. Root applied every reviewed path unchanged onto current main through `639adbce657e07f03c2ed41bd0855e6cd9a326b2`. Clean integration passed `make lint`, `make test`, and `make spec`. The executable specification reported 156 passed and seven platform-skipped cases on macOS.

No Git tag, GitHub release, container tag, image publication, moving tag, scoring change, asset change, or public 0.4.0 release claim entered this ticket.
