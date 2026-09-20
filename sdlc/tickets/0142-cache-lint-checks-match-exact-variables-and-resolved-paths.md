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
- Require the complete parsed shell word to equal the variable name. Apply that rule to every live cache-isolation checker, including `model-cache-limit-inheritance.sh`; a regex boundary that accepts a `-OLD` suffix does not satisfy the ticket.
- Normalize absolute paths produced after the check's supported Make-variable expansion before comparing cache locations with removed directories. Fixtures cover `.` and `..` in both operands and prove both inside and outside classifications.
- Preserve the current supported relative `ORT_CACHE_DIR` form as an explicit unclassified exception. Cargo resolves it from the external dependency package root, which this repository check does not know. Do not pretend that the repository root is its base. Reject unsafe absolute syntax or an absolute path that cannot be normalized lexically with a named message. Do not access the path or require it to exist.
- Parse the supported assignment and path forms before expansion. Reject command substitution, process substitution, shell evaluation, and glob syntax before any evaluator or word loop can execute or expand them. Prefer bounded static expansion of the admitted Make variables over `bash -c`.
- Preserve current accepted recipes. Keep the checks portable across the supported macOS and Linux shells.
- Archive drafts 0107 and 0108. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change repository checks and their fixtures only. Do not change product cache behavior, runtime paths, release assets, or public APIs. The check must stay bounded by tracked repository input and must not scan external filesystem contents.
