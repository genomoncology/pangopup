---
---
# A dropped cache variable is answered by any name that extends it

`tests/recipe-spawn-cache-isolation.sh` asks whether a recipe reaching the
built executable drops each name in `named_locations`. It asks with:

    -u[[:space:]=]+$name([^_]|$)

The class exists so that `-u PANGOPUP_MODEL_CACHE_MAX_ENTRIES` is not read as
having dropped `PANGOPUP_MODEL_CACHE`, and for a name extended by `_` it does
that. Every other character answers the shorter name instead. Measured on
2026-09-11 against the shipped pattern: `-u PANGOPUP_MODEL_CACHEX`,
`-u PANGOPUP_MODEL_CACHE2` and `-u PANGOPUP_MODEL_CACHE-OLD` each satisfy the
requirement that `PANGOPUP_MODEL_CACHE` be dropped, and the variable is
inherited whole by the run.

Nothing in the repository spells such a name today, so nothing is unheld. The
exposure is a typo: a recipe written with `-u PANGOPUP_MODEL_CACHEX` passes
every gate while handing the run the operator's own model cache.

The same shape stands in `tests/cli-spawn-cache-isolation.sh` and
`tests/shell-spawn-cache-isolation.sh` wherever a variable name is searched for
with a trailing character class rather than a word boundary. A word boundary --
`([^_A-Za-z0-9]|$)` -- answers all of them.

What a successor must prove: a recipe that drops a name extending
`PANGOPUP_MODEL_CACHE` by any character is still refused for
`PANGOPUP_MODEL_CACHE`, and a recipe that drops `PANGOPUP_MODEL_CACHE` itself
is still accepted.
