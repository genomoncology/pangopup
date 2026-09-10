---
flow: build
priority: 3
---
# A busy moment while taking up a replacement does not retire a cache

Ticket 0068 made a cache probe the path before every get and put, and retire
when the file there is not the one it judged. The probe opens the replacement
to read a verdict off it. On the matching branch that open also trims the file
to the running entry limit, and a trim against a peer holding a write lock
fails as `Busy`. The probe treats every error alike, so a peer actively filling
a same-setup replacement retires a long-running service's cache for the rest
of its life. Nothing was destroyed and nothing was wrong with either file; the
cache goes cold on a moment's contention and the only lever is a restart.

The retiring is what must change, not the probing. A replacement that records
the running setup is still taken up when it can be, and every replacement that
records anything else still retires the cache and destroys nothing. Whether a
busy probe keeps the old file in hand and tries again on the next operation, or
waits, is the design stage's decision.

Beside it, the same flow carries two smaller losses the review of 0068 found.
Taking up a replacement, and recreating a default, keep the new connection and
drop the new counters, so evictions performed during that open are counted
nowhere; nothing observes this until ticket 0074 makes the crate's counters
speak. And the comment on the probe promises nothing is created to replace
what went, while a file that vanishes between the stat and the probe can be
recreated by the probe itself and then taken up; the comment overstates and
should say so.

Done, observably:

- A cache whose replacement records its own setup stays in service when that
  replacement is momentarily busy, and serves no row from any other setup
  while it waits or retries.
- A cache that retires for a cause that is not contention behaves exactly as
  ticket 0068 settled.
- A test drives a busy same-setup replacement against a running cache, so the
  difference is proved rather than reasoned about.
- Evictions performed by the open that takes up a replacement, or recreates a
  default, survive into the counters the cache already keeps.
- `make test`, `make spec`, `make lint` and the service fixture tests pass.

Boundary: this ticket changes no answer, no score, no row key, no recorded
setup, and no discard. It does not widen which replacements are taken up --
another setup, an earlier layout, and an unreadable file still retire the cache
for the life of the process. It does not change the one-stat-per-operation
cost, and it does not add any report; ticket 0074 owns what a retirement says.
