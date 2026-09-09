---
---
# Only one test file is held away from the operator's cache

`crates/pangopup-cli/tests/model_routing.rs` now gives every run of the shipped executable a private cache home, and `no_run_here_reaches_the_ambient_model_cache` proves it. That proof reaches one file. Nothing stops the next test file from spawning `pangopup` against the cache home the suite inherited.

Measured in this checkout on 2026-09-09.

- Four test sources name `--model-bundle`: `crates/pangopup-cli/tests/model_routing.rs`, `model_cache_setup.rs`, `http_service_lifecycle.rs` and `macos_cli_boundary.rs`. The first three redirect `XDG_CACHE_HOME` and `HOME`. The fourth spawns through a helper that redirects neither, and reaches the model cache only because its `--model-bundle` belongs to `assets runtime install` rather than to `lookup`.
- Running each `pangopup-cli` test target under a scratch `XDG_CACHE_HOME` left `pangopup/model-results.sqlite3` there for `model_routing` alone. Every other target left the directory empty.
- `make test` sets no `XDG_CACHE_HOME`, so whatever a test inherits is the operator's own. Redirecting it for the whole `cargo test` step is not free: the ONNX Runtime library lands under `XDG_CACHE_HOME` at link time, 87 MB of it, so moving the variable moves that too and clearing the directory would re-fetch it.
- `.github/workflows/ci.yml:185` runs `cargo test --locked --package pangopup-cli --test macos_cli_boundary` with no redirection of its own.

Done, observably:

- Every test that can reach the default model cache is held to the rule, not one file.
- The rule fails when a spawn stops redirecting, and it names the spawn.
- The check counts what it examined, so a scan that matches nothing fails rather than passes.
- `make lint`, `make test` and `make spec` pass, and `make test` wall time does not grow by more than two seconds.
