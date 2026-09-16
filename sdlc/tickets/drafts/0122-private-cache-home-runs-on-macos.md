---
flow: build
priority: 5
---
# The private cache-home helper runs on macOS and Linux

At the current source commit, macOS `make test` passes the build-residue check and then stops in `tests/inherited-cache-variables.sh` with `chmod: --: No such file or directory`. The check sources `tests/support/private-cache-home.sh`, whose permission-setting command uses a GNU-only argument form. This prevents the test from proving that inherited cache variables cannot reach an operator's own files.

The shared shell helper must create an owner-private cache home on both macOS and Linux. It must still move `HOME` and `XDG_CACHE_HOME`, preserve the caller's Cargo and Rust toolchain homes, drop inherited PangoPup cache variables, and leave product-level cache settings available to an operator outside a test harness.

Done, observably:

- `tests/inherited-cache-variables.sh` passes on macOS and Linux. Its checks still show that a helper-run model lookup leaves the stand-in operator cache unchanged while filling the helper's private cache.
- The helper's own permission check proves that its private cache home is owner-only even under a permissive caller umask.
- Mac `make test` advances beyond `tests/inherited-cache-variables.sh` without a `chmod` syntax error.
- Run `make lint`, `make test`, and `make spec` before commit and record remaining unrelated Mac gate failures honestly.

Boundary: do not change scoring, cache schema, operator environment precedence, model fixtures, or other Mac spec failures. Record Mac and Linux evidence in `sdlc/records/`; no public score or operator documentation changes.
