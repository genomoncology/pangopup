---
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
code fix, and 0059 declined to rewrite guidance in passing. A successor states
in the contract that a gene's score can depend on the other same-strand genes
overlapping the same variant, names the one record in the published set where
that changes an answer, and holds the statement to the corpus case that pins the
behaviour.
