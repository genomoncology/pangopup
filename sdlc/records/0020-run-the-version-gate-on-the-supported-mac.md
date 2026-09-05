---
base: 5e2894ea4b2a797bbc758fa3969f7c0933a22d02
head: 78b86f6acc7ea50b4c18022d80939201a74e41d4
---

# The version gate runs on the supported Mac

PangoPup's version consistency gate now runs under the supported Apple Silicon machine's Python 3.9.6. The checker no longer imports `tomllib`. Narrow readers extract only the workspace package version and the name and version from each Cargo lockfile package table. They require one simple quoted value and reject missing, duplicate, escaped, non-string, decoy, and changed facts.

Red commit `f58c1031c64ecfd3424077ca58f85541a2587cdb` added the portable public-path test. It failed at the existing `import tomllib` with `ModuleNotFoundError` under `/usr/bin/python3`. Candidate `78b86f6acc7ea50b4c18022d80939201a74e41d4` passes that test and rejects workspace and lockfile version mutations while `tomllib` imports are forced absent.

Independent code review accepted the exact candidate with no findings. The reviewer confirmed that all ticket 0017 candidate, current-public, history, and fixed-fixture checks remain unchanged. `timeout 295 make lint`, `timeout 295 make test`, and `timeout 295 make spec` passed. The specification reported 156 passed and seven platform skips on macOS.

No version, scoring behavior, asset, request, response, tag, image, release, or publication state changed.
