---
flow: build
priority: 1
---
# The cache is discarded when the setup that filled it changes

The model result cache is on by default and survives an upgrade in place. Nothing in it records which software or which assets produced the rows it holds, so a row written by one setup answers a request made under another. The cache can return a splice score the running setup would never compute, and nothing tells anyone it happened.

Measured in this checkout on 2026-09-09.

- `CacheIdentity` (`crates/pangopup-cache/src/lib.rs:62`) hashes nine components into every row key. None is the PangoPup version. `USER_VERSION` and `VALUE_SCHEMA` (`:23`, `:24`) version the SQLite layout and the stored value shape, not the software.
- The cache is on by default at `$XDG_CACHE_HOME/pangopup/model-results.sqlite3` (`crates/pangopup-cli/src/main.rs:1776`), so an upgrade reads rows the previous version wrote.
- `architecture/compatibility.md:66` already publishes that "a version change can move an answer." The cache ignores the statement the repository makes to consumers.
- One of the nine components is the effective CPU policy, which the service builds from `--model-threads`. Ticket 0040 measured that no thread or worker setting moves any score, position, status, reason or provenance field, over eighteen variants and twenty-three gene records across six pairwise comparisons. The key therefore discards every paid-for row on a thread change while failing to discard on a software change. It is wrong in both directions at once.
- The two callers disagree with each other. `crates/pangopup-cli/src/main.rs:1052` keys on `CpuPolicy::production_default()` and `crates/pangopup-cli/src/service.rs:815` keys on the effective policy, so the command-line tool and the service write different rows for the same variant on the same assets.
- Assets are chosen at run time, not at build time. The command-line cache path requires `--model-bundle`, `--reference-bundle` and `--mask` (`crates/pangopup-cli/src/main.rs:1717`), and the service takes `--data-dir`. One binary reaches different references and masks by flag, so a release number alone cannot describe what filled a cache.

Settled: the cache is a short-lived shortcut, not a durable store. When the setup that filled it no longer matches the setup asking, the whole file is discarded and refilled. No migration keeps old rows readable and none is wanted. Losing a cache costs re-inference. Serving a row from another setup costs a wrong splice score with no signal, and that is worse.

Settled: what the cache is stamped with identifies the setup, not the release, for the reason measured above.

Settled: the row key is the submitted variant. The setup is judged once, when the cache opens.

Done, observably:

- A cache file records the setup that filled it, and a reader can see what it records.
- Opening a cache whose recorded setup does not match the running setup leaves no earlier row readable. The next request for one of those variants recomputes.
- A cached row is found by the submitted variant alone. A test scores a variant, changes only the thread count, and observes a hit rather than a miss.
- A change of software version, of model, of reference, or of mask each discards the file on its own. A test proves the four separately.
- The command-line tool and the service hit each other's rows when both run the same setup, and neither hits the other's rows when they do not.
- A discarded cache is reported, not silent, so an operator can tell a cold cache from a broken one.
- `make lint`, `make test` and `make spec` pass. `make test` wall time does not grow by more than two seconds.

Boundary: this ticket changes what a cached row is found by, what the cache file records about itself, and when the file is discarded. It must not change any score value, position, status, reason or provenance field on either route.

It must not change `scoring_identity` or `data_set_version`, what the status response publishes, or what an HTTP score item carries. Ticket 0052 settled those and they stay exactly as they are.

It must not change where the cache lives, that it is on by default, the `--model-cache` and `--model-cache-max-entries` flags, or the disposable-default behavior at `crates/pangopup-cli/src/main.rs:1776`.

It must not change the precomputed lookup route, which consults no model cache and is not affected by any of this.

It must not change what `pangopup lookup` prints. What a retained command-line score pins is ticket 0054, and this ticket must leave that question open.
