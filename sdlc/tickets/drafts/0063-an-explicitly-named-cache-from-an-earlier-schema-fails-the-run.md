---
---
# An explicitly named cache from an earlier schema fails the run

Closed inside ticket 0058 by its code review. `USER_VERSION` now stamps the file layout, and a file carrying a layout an earlier release wrote is discarded whole and reported, the way any other setup change is, instead of failing the run. A foreign, damaged or later-layout database is still refused rather than deleted. Measured: a `--model-cache` path holding a v0.4.x cache now exits 0, prints one score line, names the discarded file on standard error, and refills. Nothing below is still open.

Ticket 0058 discards a cache file whose recorded setup is not the running setup, and it changed the shape of the cache file to hold that record. A file an earlier release wrote has no setup table at all, so it is not a setup mismatch. It is a schema mismatch, and an explicitly named database is never deleted on one. The run stops instead of throwing the file away and refilling it.

Measured in this checkout on 2026-09-09.

- `pangopup lookup --model-only ... --model-cache <path>` filled a chosen cache, the `setup` table was removed from that file, and the same command re-run exited 1 with `{"status":"error","code":"MODEL_CACHE_INVALID","message":"model cache schema is incompatible"}`.
- `ModelResultCache::open_explicit` (`crates/pangopup-cache/src/lib.rs`) returns `CacheError::Incompatible` for a schema it did not write, and `map_cache_error` (`crates/pangopup-cli/src/main.rs:1382`) turns that into an exit-1 failure.
- The disposable default is unaffected: `open_default` removes an incompatible file and recreates it.
- ADR 0019 states that an explicitly selected database is never silently replaced. Refusing is one way to honour that. Reporting a discard, which 0058 already added for a setup change, is another.

Done, observably:

- An operator who upgrades while naming a cache path gets an answer, not a failed run.
- Whatever happens to the earlier file is reported, so nothing is replaced silently.
- A genuinely corrupt or foreign explicitly named database still behaves the way ADR 0019 says it does.
- `make lint`, `make test` and `make spec` pass.
