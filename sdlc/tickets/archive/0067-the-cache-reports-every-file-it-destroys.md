---
flow: build
priority: 2
---
# The cache reports every file it destroys, and its layout list cannot rot

Ticket 0058 requires that an operator can tell a cold cache from a broken one. It met that for a setup change and left two holes beside it. A default cache destroyed because it could not be read still says nothing, and the hand-kept list that decides which files are readable has no gate behind it. The two belong together: the list decides which files reach the destruction that goes unreported.

## The silent destruction

Measured in this checkout on 2026-09-09 against a redirected `XDG_CACHE_HOME`.

A default cache holding two rows was overwritten at its head with `not sqlite at all`. The next `pangopup lookup --model-only` exited 0, wrote one row, and printed nothing on standard error. The same happened for a file stamped with a foreign `application_id`, and for one stamped with a `user_version` this build does not know.

`ModelResultCache::open_default` (`crates/pangopup-cache/src/lib.rs`) catches `CacheError::Incompatible` and `CacheError::Sqlite`, calls `remove_database_family`, and reopens. The reopened cache carries `discarded == false`, so the caller has nothing to report. The setup-change discard beside it does set `discarded`, and both callers report it by name. Two destructions of the same file differ only in what caused them, and only one is spoken.

The third case is the one that will be met in practice. A cache written by a later release, opened by an earlier binary, is destroyed without a word.

Ticket 0058's Boundary forbade changing the disposable-default behavior, so the silence was left alone when the report was added.

## The list nothing holds

`crates/pangopup-cache/src/lib.rs:23` holds `USER_VERSION` at 2 and `:27` holds `EARLIER_USER_VERSIONS` at `[1]`. That constant appears at exactly two places in the whole checkout, its own definition and its one use at `:466`. No test, script or spec reads it, so no gate can notice a layout bump that leaves it alone.

Ticket 0058 changed the on-disk shape and left `USER_VERSION` at 1. `make lint`, `make test` and `make spec` all passed on that change. A reader found it, not a gate.

The cost when it happens again is measured, because it already happened once: a bump to layout 3 that does not append 2 makes every cache a v0.5.x release wrote fail an explicitly named `--model-cache` path with `MODEL_CACHE_INVALID` and exit 1, and be destroyed with no report on the default path. Those are the two defects commit `e7bfa6a` repaired.

The tests authored for 0058 do not close this. `a_default_cache_an_earlier_release_wrote_is_discarded_and_reported` and `a_chosen_cache_an_earlier_release_wrote_is_discarded_rather_than_fatal` in `crates/pangopup-cli/tests/model_cache_setup.rs` build a frozen layout-1 fixture copied from tag v0.4.1. They keep proving layout 1 after a bump to 3, and say nothing about layout 2.

Done, observably:

- A default cache destroyed because it could not be read reports that, naming the file, the way a setup change already does.
- A cold open and a matching open still say nothing.
- An explicitly named database that cannot be read still refuses rather than being deleted.
- A change that moves `USER_VERSION` without naming the layout it replaced fails a gate, and the gate says which layout went unnamed.
- That gate counts what it examined, so a scan matching nothing fails rather than passes.
- The layout each tagged release wrote is exercised, not only the newest one before this build.
- `make lint`, `make test` and `make spec` pass.

Boundary: this ticket changes what the cache says when it destroys a file, and adds a gate holding the layout list to the release history. It must not change what a matching, foreign, damaged or later-layout file does today, and it must not add a migration. The whole file is still discarded and no earlier row is kept readable.

It must not change where the cache lives, that it is on by default, the `--model-cache` and `--model-cache-max-entries` flags, or the row key. Ticket 0058 settled those.
