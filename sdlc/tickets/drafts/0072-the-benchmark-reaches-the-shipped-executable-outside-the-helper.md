---
priority: 6
---
# The benchmark reaches the shipped executable outside the helper

Ticket 0069 holds `crates/pangopup-cli/tests/` to spawning the shipped executable through `crates/pangopup-cli/tests/support/mod.rs`. `crates/pangopup-cli/benches/snv_regression.rs` reaches the same executable and is not held to it: `cli_path()` walks from `env::current_exe()` to `target/debug/pangopup`, builds it, and the `fresh-process` sample spawns it with no cache home of its own.

Measured in this checkout on 2026-09-09, that spawn reaches no cache. It passes `lookup --bundle`, and a default model cache is resolved only when `--model-bundle`, `--reference-bundle` and `--mask` are supplied together. That is the same accident 0069 named on the test side: it holds because of which arguments the call happens to carry, not because anything requires it. A benchmark that grew a modelled route would fill the operator's own cache with rows scored from fixtures, and 0058 discards a cache whose setup no longer matches rather than ignoring it.

0069's gate cannot see it. Its detour scan covers only the test tree the helper serves, because outside that tree `current_exe` and `target/debug/pangopup` have honest uses: `crates/pangopup-cli/src/uninstall.rs` reports the running executable's own path, and `crates/pangopup-build/tests/transport_resources.rs` re-executes its own test binary.

Done, observably:

- A benchmark cannot run the shipped executable without a cache home of its own.
- The check counts what it read, so a scan that matches nothing fails rather than passes.
- The refusal names the file and line.

Boundary: no product behavior, cache location, default, or scoring assertion changes. `cargo bench` must keep building the executable it times; a benchmark that ran a stale binary would report the wrong number.
