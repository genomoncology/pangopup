---
flow: build
priority: 1
deps: []
---
# Cache durability checks reject multiple command lists

## Outcome

The static cache-durability check refuses a relevant Make recipe line containing more than one shell command. A later command cannot hide its cache environment.

## Done, observably

- Reject an unquoted, unescaped semicolon, `&&`, `||`, background separator, or pipeline on a relevant recipe line with a path-and-line diagnostic. A line shaped like `true; env ORT_CACHE_DIR="$(CURDIR)/target/spec-cache/ort" cargo build` must fail for that reason.
- Preserve quoted and escaped separator characters as ordinary argument text.
- Preserve exact cache-variable matching, static environment-prefix handling, lexical path normalization, the relative `ORT_CACHE_DIR` exception, and every accepted or rejected case from ticket 0142.
- Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the cache-durability checker and one focused separator fixture only. Do not parse multiple commands, implement a general shell parser, alter product behavior, execute recipe text, or broaden allowed cache layouts.
