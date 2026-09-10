---
flow: build
priority: 3
---
# Nothing says a score depends on the other genes scored beside it


`crates/pangopup-engine/src/lib.rs`, `score_typed`, walks the genes overlapping
a variant and calls `apply_mask` on shared `gain` and `loss` slices. The mask
one gene applies is still in place when the next same-strand gene is scored, so
a gene's answer depends on which other same-strand genes were scored before it.
Nothing a consumer reads says that.

The behaviour is upstream-faithful and the corpus pins it as
`P01-same-strand-order`, so it is not a defect against the contract. It is rare:
a patched build that computed both the shipped cumulative answer and an isolated
per-gene answer for every record of the 2,615-variant `snv-stride-500k-v1` set
found 3 records out of 3,038 where the two differ, and only one of those changes
a score a consumer would read. That one is `chr19:51000000:G:T` /
`ENSG00000269741`: the cumulative answer is `gain 0.00`, the isolated answer is
`gain 0.09 at -4`, which is the published dataset's exact value. It is the sole
cause of the second value disagreement the 0059 measurement published.

Ticket 0059 measured and published the rate; this gap is documentation, not a
code fix, and 0059 declined to rewrite guidance in passing.

Done, observably:

- The published contract states that a gene's score can depend on which other
  same-strand genes overlap the same variant, in the document a consumer reads
  to learn what a score means.
- The statement names the one record in the published measurement set where the
  dependence changes an answer a consumer would read, so a reader can see the
  size of the effect rather than guess at it.
- The statement is held to the frozen corpus case that pins the behaviour, so a
  change in the behaviour and a change in the words cannot part company.
- `make test`, `make spec` and `make lint` pass.

Boundary: this is documentation. It changes no product source under `crates/`,
does not alter `score_typed`, the masking order, or any score, position, status,
reason or provenance. It does not change the frozen corpus, the oracles under
`tests/fixtures/` or their pinned digests, and it does not revisit the rate
ticket 0059 published.
