---
priority: 6
---
# The shell qualification harnesses reach the executable unwatched

Ticket 0069 holds the Rust suite to spawning the shipped executable through one helper. Its gate reads `*.rs` only. `make test` also runs shell harnesses that run `target/debug/pangopup` directly: `tests/executable-delivery.sh`, `tests/production-release-qualification.sh` and `scripts/smoke-linux-release.sh` on Linux, and `tests/release-help-contract.sh` on every platform.

Measured in this checkout on 2026-09-09, none of them reaches the operator's cache. Every invocation that supplies `--model-bundle`, `--reference-bundle` and `--mask` together — the only lookup route that resolves a default model cache — either passes `--model-cache` explicitly or sets `XDG_CACHE_HOME` for that command. The rest never resolve one. That is the same accident 0069 named on the Rust side: it holds because of which arguments each call happens to carry, not because anything requires it.

Done, observably:

- A shell harness cannot run the shipped executable on a cache-resolving route without a cache home of its own.
- The check counts the invocations it read, so a scan that matches nothing fails rather than passes.
- The refusal names the file and line.

Boundary: no product behavior, cache location, default, or scoring assertion changes. It must not move `XDG_CACHE_HOME` for the whole `make test` step; the ONNX Runtime library lands under that variable at link time.
