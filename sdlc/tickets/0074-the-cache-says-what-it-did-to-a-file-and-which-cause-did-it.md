---
flow: build
priority: 2
---
# The cache says what it did to a file, and which cause did it

Ticket 0067 gave an operator two sentences at open, so a cold cache can be told
from a destroyed one. Three events still say the wrong thing or nothing at all,
and all three mislead the same operator in the same way: a cache that was warm
goes cold, and nothing on any stream says why or what the lever is.

**A file discarded for its layout is reported as another setup.** `ModelResultCache::replace`
raises one flag for two causes, so `discarded_earlier_setup` is true whether the
file recorded another setup or an earlier layout, and both entry points print the
setup sentence. Measured on 2026-09-10 in this checkout: a `lookup` over a cache
carrying an earlier `user_version` printed `discarded model cache <path>: another
setup filled it`, then restamped the file. Nothing about the assets changed. What
changed was the release. The operator is sent after an upgrade nobody made and
away from the release note that explains it. Ticket 0067 split the report in two
for exactly this reason and stopped one cause short.

**A file destroyed mid-run is destroyed in silence.** `ModelResultCache::get`
catches a SQLite failure on a disposable default cache and calls `recreate_default`,
which drops the connection, removes the file family and reopens it empty. It sets
neither flag. Both entry points read the flags once, immediately after opening and
before any lookup runs, so nothing would print even if a flag were set. This is
reached when the file becomes unreadable after it was opened: another process
overwrites it, the disk damages it, or a lookup trips an error the open never saw.
The whole file is gone and the operator is told a cold cache.

**A process that retires its cache says nothing.** Ticket 0068 makes a process stop
answering from, and stop writing to, a file another setup replaced underneath it,
for the rest of its life. Stopping is the right answer and it is silent. Nothing is
destroyed here and the replacement is left untouched, so this is not a destruction
report — it is the fourth event, and the counters that would show it are private to
the crate. A long-running service goes cold and stays cold with no upgrade, no
restart and no error. The lever is a restart, and nothing says so.

The report is written when the event happens, not collected and printed later. A
run that destroys or retires says so at that moment, on the stream the existing
reports use, naming the file. The service has no report point after startup and
gains one; whether that is per connection, per worker or per event is the design
stage's decision, but each event is reported once rather than once per lookup that
follows it.

Done, observably:

- A cache discarded because this build reads its layout as an earlier one says so,
  naming the layout as the cause, and does not say another setup filled it.
- A default cache that becomes unreadable after it was opened is reported when it
  is destroyed, naming the file, from the command line and from a running service
  alike, and is not confused with either cause reported at open.
- A process that stops using a cache file because another setup replaced it, or
  because the path no longer names a file it can open, says so once for the
  process, naming the file.
- A process that takes up a replacement recording its own setup goes on working
  and says nothing, because nothing was lost. A process whose file is never
  replaced still says nothing. A run that destroys nothing mid-run stays silent.
- The two sentences ticket 0067 settled are unchanged in wording and in cause, and
  the service and the command line say the same thing for the same cause.
- A test drives a cache that fails a read after a successful open, so that path is
  proved rather than reasoned about.
- `make test`, `make spec`, `make lint` and the service fixture tests pass.

Boundary: this ticket changes no answer, no score, position, status, reason or
provenance. It does not change when a file is destroyed, which files are destroyed,
or what is destroyed — the mid-run recreate stays exactly as it is. It does not
widen or narrow which replacements a process takes up or when it retires; ticket
0068 settles that. It does not change the row key, the recorded setup, the cache
location, the default, or the `--model-cache` and `--model-cache-max-entries`
flags, and it does not make a disposable cache refuse where it recreates today.
