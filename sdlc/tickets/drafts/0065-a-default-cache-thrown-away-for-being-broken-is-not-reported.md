---
---
# A default cache thrown away for being broken is not reported

Ticket 0058 says an operator must be able to tell a cold cache from a broken one, and the discard it added reports itself. One older path does not. `ModelResultCache::open_default` deletes and recreates a file it cannot read, and says nothing at all. The run answers normally and the operator never learns that a cache was thrown away.

Measured in this checkout on 2026-09-09, against a redirected `XDG_CACHE_HOME`.

- A default cache holding two rows was overwritten at its head with `not sqlite at all`. The next `pangopup lookup --model-only` exited 0, wrote one row, and printed nothing on standard error. The same happened for a file stamped with a foreign `application_id` and for one stamped with a `user_version` this build does not know.
- `open_default` (`crates/pangopup-cache/src/lib.rs`) catches `CacheError::Incompatible` and `CacheError::Sqlite`, calls `remove_database_family`, and reopens. The reopened cache carries `discarded == false`, so the caller has nothing to report.
- The setup-change discard beside it does set `discarded`, and both callers report it by name. The two destructions of the same file differ only in what caused them.
- Ticket 0058's Boundary forbids changing the disposable-default behavior, so the silence was left alone when the report was added.

The third case above is the one that will be met in practice: a cache written by a later release, opened by an earlier binary, is destroyed without a word.

Done, observably:

- A default cache destroyed because it could not be read reports that, naming the file, the way a setup change does.
- A cold open and a matching open still say nothing.
- An explicitly named database that cannot be read still refuses rather than being deleted.
- `make lint`, `make test` and `make spec` pass.
