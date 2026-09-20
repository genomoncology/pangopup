---
flow: build
priority: 1
deps: []
---
# Cache lint checks match exact variables and resolved paths

## Outcome

Repository checks decide cache isolation from the exact environment variable and the resolved filesystem location. A misspelled longer variable cannot stand in for the required variable. Dot and parent path segments cannot reverse whether routine cleanup removes a downloaded cache.

## Done, observably

- Add inside-out fixtures for both live variable matchers. Each accepts an exact `PANGOPUP_MODEL_CACHE` removal and rejects removals of longer names such as `PANGOPUP_MODEL_CACHEX`, `PANGOPUP_MODEL_CACHE2`, and `PANGOPUP_MODEL_CACHE-OLD`.
- Require the complete parsed shell option operand to equal the variable name in `recipe-spawn-cache-isolation.sh` and `model-cache-limit-inheritance.sh`. A regex boundary that accepts a `-OLD` suffix does not satisfy the ticket. Do not mechanically change unrelated executable-name matching in the CLI and shell spawn checks.
- Normalize absolute cache paths after the check's supported Make-variable expansion and normalize relative removal operands against the repository root before comparison. Fixtures cover `.` and `..` in both operands, equal paths, sibling prefixes, trailing separators, and equivalent spellings on separate build lines.
- Preserve the current supported relative `ORT_CACHE_DIR` form as an explicit unclassified exception. Cargo resolves it from the external dependency package root, which this repository check does not know. Do not pretend that the repository root is its base. Reject unsafe absolute syntax or an absolute path that cannot be normalized lexically with a named message. Do not access the path or require it to exist.
- Parse the supported assignment and path forms before expansion. Reject command substitution, backticks, process substitution, shell evaluation, wildcard operands, and unknown variables before any evaluator or word loop can execute or expand them. Refusal fixtures leave sentinel files untouched. Use bounded static expansion of admitted Make variables instead of `bash -c`.
- Preserve current accepted recipes. Keep the checks portable across the supported macOS and Linux shells.
- Archive drafts 0107 and 0108. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change repository checks and their fixtures only. Do not change product cache behavior, runtime paths, release assets, or public APIs. The check must stay bounded by tracked repository input and must not scan external filesystem contents. Path resolution is lexical only; it does not resolve symlinks or require a path to exist. Keep Bash 3.2 support and do not add GNU-only path tools.
