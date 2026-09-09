---
flow: build
priority: 4
---
# Every test that can reach the operator's cache is held to the rule

`crates/pangopup-cli/tests/model_routing.rs` now gives every run of the shipped executable a private cache home, and `no_run_here_reaches_the_ambient_model_cache` proves it. That proof reaches one file. Nothing stops the next test file from spawning `pangopup` against the cache home the suite inherited, which on a developer machine is the operator's own.

This matters more since ticket 0058. Before it, a stray test run added fixture rows to a real cache and the cost was a file that grew. After it, a stray run under fixture assets discards whatever that cache held.

Measured in this checkout on 2026-09-09.

Four test sources name `--model-bundle`: `crates/pangopup-cli/tests/model_routing.rs`, `model_cache_setup.rs`, `http_service_lifecycle.rs` and `macos_cli_boundary.rs`. The first three redirect `XDG_CACHE_HOME` and `HOME`. The fourth spawns through a helper that redirects neither, and reaches no model cache today only because its `--model-bundle` belongs to `assets runtime install` rather than to `lookup`. That is an accident of which subcommand it happens to call, not a rule.

Running each `pangopup-cli` test target under a scratch `XDG_CACHE_HOME` left `pangopup/model-results.sqlite3` there for `model_routing` alone. Every other target left the directory empty. `make test` sets no `XDG_CACHE_HOME`, so whatever a test inherits is the operator's own. `.github/workflows/ci.yml:185` runs `cargo test --locked --package pangopup-cli --test macos_cli_boundary` with no redirection of its own.

Settled: redirecting `XDG_CACHE_HOME` for the whole `cargo test` step is not the answer. The ONNX Runtime library lands under that variable at link time, 87 MB of it, so moving the variable moves that too and clearing the directory would re-fetch it. The design review of 0058 measured this and rejected it.

Done, observably:

- Every test that can reach the default model cache is held to the rule, not one file.
- The rule fails when a spawn stops redirecting, and the refusal names the spawn.
- The check counts what it examined, so a scan that matches nothing fails rather than passes.
- A full `make test` writes nothing under the operator's `XDG_CACHE_HOME` or `HOME`.
- `make lint`, `make test` and `make spec` pass, and `make test` wall time does not grow by more than two seconds.

Boundary: this ticket holds the test suite away from the operator's cache. It must not change any product behavior, where the cache lives, that it is on by default, or what any test asserts about scoring.

It must not move `XDG_CACHE_HOME` for the whole `cargo test` step, for the reason settled above. It must not undo the per-spawn redirection commit `5393f44` added to `model_routing.rs`.
