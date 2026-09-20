---
flow: build
priority: 1
deps: []
---
# Cache lint checks match exact variables and resolved paths

## Outcome

Repository checks decide cache isolation from the exact environment variable and the resolved filesystem location. A misspelled longer variable cannot stand in for the required variable. Dot and parent path segments cannot reverse whether routine cleanup removes a downloaded cache.

## Done, observably

- Add an inside-out fixture that accepts an exact `PANGOPUP_MODEL_CACHE` removal and rejects removals of longer names such as `PANGOPUP_MODEL_CACHEX`, `PANGOPUP_MODEL_CACHE2`, and `PANGOPUP_MODEL_CACHE-OLD`.
- Apply the same exact variable-token rule to every live cache-isolation checker that carries this matching shape.
- Resolve safe absolute lexical paths before comparing cache locations with removed directories. Fixtures cover `.` and `..` in both operands and prove both inside and outside classifications.
- Reject an unsafe or unresolved path with a named message. Do not access the path or require it to exist.
- Preserve current accepted recipes. Keep the checks portable across the supported macOS and Linux shells.
- Archive drafts 0107 and 0108. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change repository checks and their fixtures only. Do not change product cache behavior, runtime paths, release assets, or public APIs. The check must stay bounded by tracked repository input and must not scan external filesystem contents.
