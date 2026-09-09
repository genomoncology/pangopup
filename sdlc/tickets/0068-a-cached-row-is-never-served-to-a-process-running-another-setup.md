---
flow: build
priority: 2
---
# A cached row is never served to a process running another setup

Ticket 0058 settles that a cached row is found by the submitted variant alone, and that the setup is judged once when the cache opens. Both halves are right for one process at a time. Two processes sharing one cache file defeat them together, and the harm 0058 exists to prevent comes back through concurrency instead of through an upgrade.

Measured in this checkout on 2026-09-09.

`crates/pangopup-cli/src/service.rs` opens the cache once for the handler and once per worker at start-up, and holds every connection for the life of the service. After 0058 the row key carries no asset identity, so a row is found by its variant and the setup is checked only when the file opens.

That produces this sequence. A service opens the cache under one setup. A command-line run then opens the same file under another setup, finds the mismatch, discards the file whole, records its own setup, and fills it with its own rows. The service's connections stay open, see the refilled table, and find rows for submitted variants that were scored under the other setup. The service answers from them.

The default cache path is shared, so this needs no unusual configuration. A service left running against the default cache and a `lookup` run with other assets and no `--model-cache` reach the same file.

Settled: this is not a reason to reopen what 0058 settled. Judging the setup once per open is what makes a hit cheap, and re-reading the recorded setup on every hit would undo that. The answer has to hold the guarantee without paying that cost on every row.

Done, observably:

- A row written under one setup is never served by a process running another, whatever else holds the file open.
- A test starts a service, discards its cache from a second process running other assets, asks the service for a variant that second process cached, and observes a recomputed answer rather than the stored one.
- A single process with the file to itself still hits every row it wrote, and a thread-count change still discards nothing. Ticket 0058 settled both and they stay true.
- `make lint`, `make test` and `make spec` pass. `make test` wall time does not grow by more than two seconds.

Boundary: this ticket changes what a process trusts about a cache file another process may have replaced underneath it. It must not change the row key, what the cache records about its setup, when a file is discarded, or that a discard is reported. Ticket 0058 settled all of those.

It must not change any score value, position, status, reason or provenance field, and it must not change where the cache lives, that it is on by default, or the `--model-cache` flags.
