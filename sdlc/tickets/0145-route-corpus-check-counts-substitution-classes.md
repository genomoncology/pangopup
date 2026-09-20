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
- Refuse a malformed, equal-base, or non-SNV corpus row with a substitution-class diagnostic.
- Compare the counts with the existing retained artifact and public wording. One fixture changes a matching submitted and raw-record variant into a transition and reaches this assertion instead of an earlier set-membership failure.
- Run the existing route-corpus check from `make lint`. Do not add a second corpus scanner.
- Preserve the current 2,615-transversion, zero-transition result and its four directed substitution counts.
- Archive draft 0105. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the existing committed-corpus check, one focused mutation fixture, and the matching count wording only. Do not create a new parser, run the model, change the corpus, or broaden the claim.
