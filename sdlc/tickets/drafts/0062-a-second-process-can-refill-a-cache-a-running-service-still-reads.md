---
---
# A second process can refill a cache a running service still reads

Ticket 0058 settles that a cached row is found by the submitted variant alone and that the setup is judged once, when the cache opens. Both halves are right for one process at a time. Two processes sharing one cache file can defeat them together.

Measured in this checkout on 2026-09-09, against the design 0058 authorizes rather than against shipped code.

- `crates/pangopup-cli/src/service.rs` opens the cache once for the handler and once per worker at start-up and holds every connection for the life of the service.
- After 0058, the row key carries no asset identity. A row is found by its variant, and the setup is checked only when the file opens.
- So a service that opened under one setup, and a command-line run that then opens the same file under another, produce this: the second process finds a mismatch, discards the file whole, records its own setup, and fills it with its own rows. The service's connections stay open, see the refilled table, and find a row for a submitted variant that was scored under the other setup.
- The default cache path is shared. A service left running with the default cache and a `lookup` run with other assets and no `--model-cache` reach the same file.

This is the harm ticket 0058 exists to prevent, reappearing through concurrency rather than through an upgrade. It is not a reason to reopen what 0058 settled: judging the setup once per open is what makes a hit cheap, and re-reading the record on every hit would undo that.

Done, observably:

- A row written by one setup is never served to a process running another, whatever else has the file open.
- A test starts a service, discards its cache from a second process running other assets, asks the service for a variant that second process cached, and observes a recomputed answer rather than the stored one.
- `make lint`, `make test` and `make spec` pass.
