---
base: 32b89aaf650ee3fa408722c1ac492d9408d3725e
head: 91c615632a24d6e5bc975949dc51c18c05231d3d
---
# A cached row is never served to a process running another setup

A cache now retires the moment the file it judged is no longer the file at its path. Every `get` becomes a miss and every `put` a no-op, for the life of the process, so a running service can no longer answer out of a file a second process discarded and replaced underneath it. The setup is still judged exactly once per open, as ticket 0058 settled; the added cost is one `stat` per operation and none per row.

A replacement that records the running setup is taken up instead of retiring, so a peer repairing a file it could not read does not cost a long-running service its cache. Every other replacement -- another setup, an earlier layout, an unreadable file, a path that no longer names a file -- retires the cache and destroys nothing.

Retiring is not destroying. A cache that walks away from a file leaves it byte-identical, including its entry count: the probing open no longer trims a file it has no claim on to its own limit.

Nothing else moved. No row key, no recorded setup, no discard, no discard report, no score, no cache location, and no flag.
