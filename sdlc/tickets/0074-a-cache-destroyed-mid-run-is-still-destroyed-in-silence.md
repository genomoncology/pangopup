---
flow: build
priority: 2
---
# A cache destroyed mid-run is still destroyed in silence


Ticket 0067 made the two destructions at open speak. A third destruction, later in the same run, still says nothing.

`ModelResultCache::get` (`crates/pangopup-cache/src/lib.rs:562`) catches `CacheError::Sqlite` on a disposable default cache and calls `recreate_default` (`:632`). That function drops the connection, calls `remove_database_family` on the file, and reopens it empty. It sets neither `discarded` nor `unreadable`, so `discarded_earlier_setup()` and `discarded_unreadable_file()` both stay false and the run reports nothing. The whole file is gone and the operator is told a cold cache.

This is reached when the file becomes unreadable after it was opened: another process overwrites it, the disk damages it, or a lookup trips a SQLite error the open never saw. It is the same loss ticket 0067 exists to report, arriving through a different door.

The report is written when the destruction happens, not collected and printed
later. A run that destroys a cache says so at the moment it does it, on the same
stream and in the same shape as the two reports ticket 0067 landed, naming the
file and the reason. The service has no report point after startup and gains
one; whether that is per connection, per worker or per destruction is the design
stage's decision, but a destroyed file is reported once per destruction and not
once per request that follows it.

Done, observably:

- A default cache that becomes unreadable after it was opened is reported when
  it is destroyed, naming the file, from the command line and from a running
  service alike.
- The report says the file could not be read, and is not confused with the two
  reasons reported at open.
- A run that destroys nothing mid-run stays silent, and a run that destroys one
  file reports it once rather than once per lookup.
- A test drives a cache that fails a read after a successful open, so the path
  is proved rather than reasoned about.
- `make test`, `make spec`, `make lint` and the service fixture tests pass.

Boundary: this ticket does not change when a file is destroyed, which files are
destroyed, or what is destroyed — the mid-run recreate stays exactly as it is.
It does not change the row key, the recorded setup, the cache location, the
default, the `--model-cache` and `--model-cache-max-entries` flags, or the two
reports ticket 0067 landed at open. It changes no score, position, status,
reason or provenance, and it does not make a disposable cache refuse where it
recreates today.
