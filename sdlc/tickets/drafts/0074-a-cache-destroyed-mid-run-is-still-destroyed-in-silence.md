---
---
# A cache destroyed mid-run is still destroyed in silence

Ticket 0067 made the two destructions at open speak. A third destruction, later in the same run, still says nothing.

`ModelResultCache::get` (`crates/pangopup-cache/src/lib.rs:562`) catches `CacheError::Sqlite` on a disposable default cache and calls `recreate_default` (`:632`). That function drops the connection, calls `remove_database_family` on the file, and reopens it empty. It sets neither `discarded` nor `unreadable`, so `discarded_earlier_setup()` and `discarded_unreadable_file()` both stay false and the run reports nothing. The whole file is gone and the operator is told a cold cache.

This is reached when the file becomes unreadable after it was opened: another process overwrites it, the disk damages it, or a lookup trips a SQLite error the open never saw. It is the same loss ticket 0067 exists to report, arriving through a different door.

Reporting it needs a decision ticket 0067 did not make. Both call sites read the flags once, immediately after `open_cache`, and print before any lookup runs. A mid-run destruction happens after that line in `crates/pangopup-cli/src/main.rs:961` and after the service has started serving in `crates/pangopup-cli/src/service.rs:825`. Someone has to choose where the second report goes: a second read after the batch in the CLI, and something per-request or per-connection in the service, which has no report point after startup at all.

Nothing proves this today. No test drives a cache that fails a read after a successful open.
