---
flow: build
priority: 2
deps: []
---
# Published entropy figures name their exact derivation

## Outcome

A reader can reproduce every size in the entropy section from published inputs, or the text identifies the retained measurement and explains why rounded display values do not reproduce its exact byte total.

## Done, observably

- Reconcile the 64-byte difference between displayed bits per locus and the exact total without inventing or remeasuring a value. State the precision used for the exact retained calculation.
- Explain the separate-stream total from the retained per-stream source values. Publish those inputs when the retained artifact carries them; otherwise label the total as a retained measurement and cite its source.
- Add a focused documentation check that prevents either exact total from again being presented as arithmetic over an insufficient rounded value.
- Archive draft 0104. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

This is documentation and claim enforcement only. Do not change the index, recalculate the corpus, change accepted size decisions, or publish a new scientific result.
