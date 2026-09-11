---
---
# A fourth cache variable is still inherited by every spawn

Ticket 0090 gave both spawn helpers, the `spec` recipe and the maintainer
measurement the rule for three names: `PANGOPUP_MODEL_CACHE`,
`PANGOPUP_CACHE_DIR` and `PANGOPUP_DATA_DIR`. A fourth,
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES`, is read by
`resolve_model_cache_options` in `crates/pangopup-cli/src/main.rs` and is left
inherited everywhere.

`tests/production-release-qualification.sh:374` already clears all four for the
one command it runs, so the repository knows the name.

Measured in this checkout on 2026-09-10. Sourcing
`tests/support/private-cache-home.sh` from a shell that exported
`PANGOPUP_MODEL_CACHE_MAX_ENTRIES=not-a-limit` and then running a miniature
modelled lookup printed:

```
{"status":"error","code":"CLI_USAGE","message":"invalid model cache configuration: model cache maximum must be a positive integer or unlimited","details":null}
```

Every modelled lookup in the suite fails that way, so an operator who exported
the variable cannot run `make test` at all. A valid-but-small value is worse:
the suite runs, the cache evicts on a schedule the operator chose, and the
scoring harnesses read cache state they did not set up.

`maintainers/ticket-053/measure.py` has the same gap. `child_environment` drops
the three and hands the child the fourth, and the measurement contract asserts
on the number of rows the cache holds.

Done, observably:

- Both spawn helpers, the `spec` recipe and `child_environment` drop
  `PANGOPUP_MODEL_CACHE_MAX_ENTRIES` alongside the three.
- The checks that hold those routes read four names rather than three, and the
  name that shares a prefix with `PANGOPUP_MODEL_CACHE` is not satisfied by the
  shorter one.
- `make test`, `make spec` and `make lint` pass.

Boundary: no product behaviour, cache location, default, or scoring assertion
changes. The variable keeps working for an operator running the product.
