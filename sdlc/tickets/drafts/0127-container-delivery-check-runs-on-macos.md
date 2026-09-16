---
flow: build
priority: 5
---
# The container-delivery evidence check runs on macOS

Mac `make spec` reaches `spec/container-image.md` and fails before its invalid-digest control can exercise the intended refusal. The control assumes `/bin/true`; the qualifier also relies on GNU `realpath -m` and recognizes Linux `aarch64` but not macOS `arm64`. The complete bounded path to digest syntax validation must run on macOS and Linux. The negative control must remain local and must still prove that an invalid digest is refused before any Docker image inspection, network request, build, pull, start, or persistent cleanup target.

Done, observably:

- `bash tests/container-delivery.sh` exits 0 on macOS and Linux.
- The invalid-digest control uses a real inert executable on each supported host. Work-path handling and the native ARM64 spelling also accept macOS without weakening their existing Linux behavior.
- The control observes exit status 2 and the invalid-digest refusal. A recording Docker stub proves the qualifier issued no Docker command. The test proves that changing or removing this early refusal fails the check.
- Mac `make spec` advances beyond `spec/container-image.md`. Run `make lint`, `make test`, and `make spec`, and report any separate failure honestly.

Boundary: do not run a real container qualification, change the container image or publication workflow, contact a registry, or weaken the invalid-digest refusal. Record Mac and Linux evidence in `sdlc/records/`.
