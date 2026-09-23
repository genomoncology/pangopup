---
flow: build
priority: 1
deps: []
---
# Repair source root scanning and expose macOS spec failures

## Outcome

The release CI runs its source fingerprint tests again, and any later macOS spec failure names the failing output in a public job annotation.

## Done, observably

- Reproduce both failing Linux source fingerprint tests at commit `14bb9e7`. The scanner must distinguish a `crate::RuntimeProfileError` root type re-export from a root module.
- Keep declared module wiring in the checked projection. A path rebind or cross-crate re-export rebind still fails the existing mutation tests.
- Preserve the macOS `make spec` gate. On failure, report a bounded and escaped tail of its output in a public CI annotation while keeping the original exit status.
- Run focused checks and the repository's lint, test, and spec gates. Record the outcome here.

## Boundary

Change only the source fingerprint test scanner and CI failure reporting. Preserve score behavior, artifact fingerprints, and spec assertions.
