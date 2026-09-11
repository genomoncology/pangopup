---
---
# The inherited cache check picks its Rust suite by luck

`tests/inherited-cache-variables.sh` starts the built model-route suite from a
shell that exported the three cache variables, which is the only way to read
the Rust helper by running rather than by inspection. It finds that suite by
asking cargo for its artifacts and taking the last one:

```
cargo test --locked --no-run --message-format=json ... --test model_routing
    | python3 -c '... print(message["executable"])'
    | tail -n 1
```

Cargo emits a `compiler-artifact` message with an `executable` for the
`pangopup` binary as well as for the test, and the order of the two is not
fixed. Measured in this checkout on 2026-09-10, six consecutive runs of that
command with everything already built gave `pangopup` last three times and
`model_routing` last three times.

When `pangopup` comes last, `model_route_suite` is the product binary. It is
executable, so the guard on the next line passes; running it with no arguments
prints the usage block and exits non-zero; the stand-in cache is untouched for
the wrong reason; and the harness fails on the check that no passing test was
reported. `make test` then fails roughly half the time on a tree where nothing
is wrong. Observed twice in three runs of `make test` on 2026-09-10.

Done, observably:

- The harness selects the test executable by name rather than by position, so
  the same tree gives the same result every run.
- A selection that finds no test executable fails with that reason rather than
  running whatever else cargo reported.
- `make test` passes ten consecutive times.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes, and the harness keeps proving what it proves today.
