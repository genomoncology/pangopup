---
---
# The service does not report a cache it discarded

Closed inside ticket 0058 by its code review. `serve` now asks its first cache open whether it discarded an earlier setup and reports it by name on standard error. Only that open can discard, so three workers produce one report. Measured: a service started on another mask against a command-line-filled default cache prints one report and still emits its `listening` event on standard output; a matching restart prints nothing. Nothing below is still open. The proof is authored: `the_service_reports_the_cache_it_discarded_once_and_stays_silent_otherwise` in `crates/pangopup-cli/tests/http_service_lifecycle.rs`.

Ticket 0058 makes a cache file be discarded whole when the setup that filled it no longer matches the setup asking, and makes the command-line tool say so. The HTTP service performs the same discard and says nothing. An operator restarting a service after an upgrade cannot tell a cold cache from one that was thrown away.

Measured in this checkout on 2026-09-09.

- `crates/pangopup-cli/tests/http_service_lifecycle.rs`, `the_service_discards_a_cache_another_setup_filled`, shows a service on another mask leaving no row a command-line fill wrote. The discard reaches the service route.
- `discarded_earlier_setup` is consulted at one place, `crates/pangopup-cli/src/main.rs`, inside `complete_model_batch`. `open_cache` in `crates/pangopup-cli/src/service.rs` opens the same cache and never asks.
- The service opens the cache once for the handler and once per worker. Only the first open can discard, so a report belongs to that open and must not repeat per worker.
- Ticket 0058's Done list asks that a discarded cache be reported. Its approved design named the command-line route alone, so the service half was never authored or proved.

Done, observably:

- Starting a service against a cache another setup filled reports the discard once, naming the file.
- Starting a service against a matching or cold cache says nothing new.
- The report does not disturb the `listening` event a caller reads from standard output.
- `make lint`, `make test` and `make spec` pass, and the feature-gated service lifecycle tests pass.
