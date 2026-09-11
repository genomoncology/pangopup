---
---
# A new recipe may inherit the entry limit and every gate stays green

Ticket 0092 made four named routes drop `PANGOPUP_MODEL_CACHE_MAX_ENTRIES`:
the `spec` recipe, the `test` recipe, `crates/pangopup-cli/tests/support/mod.rs`
and `child_environment` in `maintainers/ticket-053/measure.py`.
`tests/model-cache-limit-inheritance.sh` holds those four by name.

A fifth route added later is not held. `tests/recipe-spawn-cache-isolation.sh`
is the scan that reads every Makefile recipe reaching the built executable, and
its `named_locations` array carries three names. Measured in this checkout on
2026-09-11, with a throwaway recipe appended to the `Makefile` that runs
`pangopup --version` under `XDG_CACHE_HOME` and `HOME` in `$(CURDIR)/target`:

- Dropping all four names: accepted. The scan reported 2 recipes examined
  rather than 1.
- Dropping `PANGOPUP_CACHE_DIR`, `PANGOPUP_DATA_DIR` and the entry limit but
  not `PANGOPUP_MODEL_CACHE`: refused, `this recipe reaches the built
  executable with PANGOPUP_MODEL_CACHE inherited ... Makefile:114`.
- Dropping the three older names but not the entry limit: accepted. Every gate
  stayed green, and `make test` stayed green, on a recipe that hands a run of
  the built executable whatever limit the operator exported.

The third case is the defect 0092 was written about, in a route 0092 could not
enumerate because it does not exist yet.

The array's own comment reads "the three variables that name a cache location
outright", and the entry limit names no location, so widening that array means
either renaming the category or adding a fourth rule beside it.
`tests/qualification-runner-cache-isolation.sh:34` already draws that
distinction in words. Which of the two shapes to take is the decision this
draft asks for.
