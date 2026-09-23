---
flow: build
priority: 1
deps: ["0156"]
---
# Build a valid sparse input for the format refusal test

## Outcome

The sparse candidate test reaches the builder's `SPARSE_INPUT_FORMAT` refusal with a valid sparse bundle and leaves no output or scratch files.

## Done, observably

- Reproduce `sparse_source_is_rejected_without_output_or_scratch` on Linux at pushed main `6eaf858`. Its test helper rejects the synthetic manifest before the builder runs because sparse format lacks sparse provenance.
- Give the test bundle valid sparse provenance tied to its fixed source and seed candidate. Verify the sparse bundle before invoking the builder.
- Keep manifest validation strict. Change neither product code nor checked fixtures.
- Pass the focused Linux test, the full Linux builder suite, and repository lint, test, and spec gates. Record any other failure separately.

## Boundary

Change only this integration test's synthetic sparse bundle construction.
