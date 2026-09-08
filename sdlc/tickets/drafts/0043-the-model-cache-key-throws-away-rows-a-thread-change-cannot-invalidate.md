---
---
# The model cache key throws away rows a thread change cannot invalidate

`CacheIdentity::new` takes the effective CPU policy as a component (`crates/pangopup-cache/src/lib.rs:75-116`), so `CacheKey` hashes it into every stored model row. The service builds that policy from `--model-threads` and renders it `sequential:{threads}/1` (`crates/pangopup-cli/src/service.rs:768-800`). A deployment that moves from one thread to four therefore misses every row it has already paid for. `production_worker_rechecks_sqlite_uses_exact_policy_and_holds_no_lock_during_model` in `crates/pangopup-cli/src/service_tests.rs` pins that miss as intended behavior.

Ticket 0040 measured whether the policy can change an answer. It cannot. Eighteen variants and twenty-three gene score records were scored under `sequential:1/1`, `sequential:4/1` and `sequential:8/1`, at worker counts one, two and four, against the shipped v0.5.0 production assets. Every score, position, status, reason and provenance field was identical in all six pairwise comparisons. The evidence is `planning/artifacts/0040-score-value-determinism.md`.

So the cache key carries an input that cannot change the value it keys. The cost is a cold model cache on every deployment thread change, and each miss is roughly ten seconds of inference for one variant.

The standalone CLI does not have this problem. It always keys on `CpuPolicy::production_default()` (`crates/pangopup-cli/src/main.rs:1065-1080`), so its rows survive.

Removing the component from the key invalidates every existing stored row, because the key is a digest over the whole identity. A migration that keeps existing rows readable is the harder half of the work and is the reason this is a draft rather than a one-line change.

Ticket 0041 covers the same input in `scoring_identity`. That is a published value a consumer stores. This is a private local cache. The two are separate decisions and the answers need not match.
