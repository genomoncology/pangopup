---
flow: build
priority: 1
deps: []
---
# Route corpus check counts substitution classes

## Outcome

The published statement about transition and transversion coverage follows from the committed route-disagreement set on every lint run.

## Done, observably

- Parse every submitted SNV in the committed set and count transitions and transversions from its reference and alternate bases.
- Refuse malformed, equal-base, or non-SNV records with a named diagnostic rather than silently classifying them.
- Compare the observed class counts with the retained artifact and the public coverage wording. A mutation that introduces a transition must fail until the stated coverage changes.
- Preserve the current 2,615-transversion, zero-transition result and its four directed substitution counts.
- Keep the check deterministic and linear in the small committed set.
- Archive draft 0105. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the committed-corpus check, its fixtures, and supporting documentation only. Do not run the model, change the corpus selection rule, or broaden the scientific claim.
