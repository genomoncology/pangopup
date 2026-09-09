---
---
# Nothing forces a replaced cache layout onto the earlier-layout list

The model cache recognizes its own file from an earlier release by a hand-kept list. A change that moves the layout forward has to remember to append the layout it replaced, and nothing catches the change that forgets. The last one did forget, and the defect reached code review rather than a gate.

Measured in this checkout on 2026-09-09.

- `crates/pangopup-cache/src/lib.rs:23` holds `USER_VERSION` at 2 and `:27` holds `EARLIER_USER_VERSIONS` at `[1]`.
- `EARLIER_USER_VERSIONS` appears at exactly two places in the whole checkout: its own definition and its one use at `:466`. No test, script or spec reads it, so no gate can notice a layout bump that leaves it alone.
- Ticket 0058 changed the on-disk shape and left `USER_VERSION` at 1. `make lint`, `make test` and `make spec` all passed on that change. A reader found it, not a gate.
- What that costs when it happens again: a bump to layout 3 that does not append 2 makes every cache a v0.5.x release wrote fail an explicitly named `--model-cache` path with `MODEL_CACHE_INVALID` and exit 1, and be destroyed with no report on the default path. Those are the two defects commit e7bfa6a repaired.
- The authored proof does not close this. `a_default_cache_an_earlier_release_wrote_is_discarded_and_reported` and `a_chosen_cache_an_earlier_release_wrote_is_discarded_rather_than_fatal` in `crates/pangopup-cli/tests/model_cache_setup.rs` build a frozen layout-1 fixture copied from tag v0.4.1. They keep proving layout 1 after a bump to 3 and say nothing about layout 2.

Done, observably:

- A change that moves `USER_VERSION` without naming the layout it replaced fails a gate, and the gate says which layout went unnamed.
- The layout each tagged release wrote is exercised, not only the newest one before this build.
- `make lint`, `make test` and `make spec` pass.

Boundary: this is about holding the list to the release history. It must not change what a matching, foreign, damaged or later-layout file does today, and it must not add a migration. The whole file is still discarded, and no earlier row is kept readable.
