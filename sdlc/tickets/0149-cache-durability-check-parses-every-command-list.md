---
flow: build
priority: 1
deps: []
---
# Cache durability checks parse every command list

## Outcome

A later command on one Make recipe line cannot hide the environment that decides a cache path from the static cache-durability check.

## Done, observably

- Inspect every admitted simple command after supported shell list separators. A line shaped like `true; env ORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort" cargo build` must fail when the same recipe removes `target/spec-cache`.
- Cover semicolon, `&&`, `||`, background, and pipeline separators, including quoted and escaped separator characters that remain ordinary argument text.
- Reject any command-list shape outside the bounded static grammar with a path-and-line diagnostic. Never execute recipe text.
- Preserve exact cache-variable matching, static environment-prefix handling, lexical path normalization, the relative `ORT_CACHE_DIR` exception, and every accepted or rejected case from ticket 0142.
- Keep the scan deterministic and linear in tracked recipe bytes. Add a checked work bound or retained scaling fixture that fails a repeated-rescan implementation.
- Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the cache-durability checker and its focused fixtures only. Do not implement a general shell parser, alter product behavior, execute Make recipes, or broaden the allowed cache layouts.
