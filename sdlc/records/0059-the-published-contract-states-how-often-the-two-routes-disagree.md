---
base: e024bc3d9ec6c02d6462ea1f6f366985cd940788
head: 8bf2db1160b8e695db3f07e1cb25ca6c981af70d
---

# The published contract states how often the two routes disagree

`architecture/compatibility.md` warned that a precomputed score and a modeled score are not interchangeable and then said "No rate of disagreement is claimed." The only evidence behind the warning was the frozen upstream corpus: four variants, five gene records, one disagreement. A consumer deciding whether to mix the routes had nothing to judge the risk with. The contract now states a measured rate.

Both routes were run over `snv-stride-500k-v1`, a set of 2,615 variants drawn by a rule rather than by hand: every position that is a multiple of 500,000 on chr1 through chr22, chrX and chrY, four candidate SNVs built at each, kept where the published dataset answers one of them. 6,164 positions were probed and 2,615 survived. The routes answered 2,790 gene records in common. They report a different value on 0.07 percent of those and a different position on 4.49 percent of the 379 records comparable on position, where a side is comparable only when its score is non-zero on both routes. `architecture/compatibility.md` and `spec/score-value.md` carry both figures, name the set, name the date, and say how zero scores were treated.

The denominator is disclosed rather than left to be discovered. 2,409 of the 2,790 records score zero on both sides on both routes, so 86 percent of the value denominator is two routes agreeing that nothing happened. Both documents say so and publish a second figure — 0.52 percent — over the 381 records where either route reports a non-zero score. A reader who takes 0.07 percent as the chance of a disagreeing call is corrected in the same paragraph.

The mechanism behind the position figure is published with its evidence and its limit. PangoPup takes the extremum of the raw 101-value window array and rounds afterwards; the published dataset appears to round to hundredths first and report the first position attaining the winning hundredth. Replaying that inferred rule against PangoPup's own arrays reproduces 16 of the 17 observed position disagreements. The contract says the rule is inferred and that the upstream software cannot be read from here. It also states how far the one-sided ordering reaches: a precomputed position fell at or before a modeled one on all 395 comparable sides, and the text calls that an observation over one substitution class, not a guarantee to build on.

Every compared record landed with the measurement, so a later reader can interrogate the number without re-running it. `planning/artifacts/0059-route-disagreement-records.tsv` holds all 2,790 rows with both routes' scores and positions; `planning/artifacts/0059-route-disagreement-set.tsv` holds the 2,615-variant manifest; `planning/artifacts/0059-route-disagreement-rate.md` holds the method, the host, the bundle digests and the machine-readable block. `tests/route-disagreement-rate.sh`, in `make test`, recomputes all nine counts from the records, checks them against the block, checks that both published documents report the same figures, and checks the ordering claim on every comparable side. The published number and its evidence cannot drift apart.

The boundary held. No crate source changed, no fixture, no oracle, no pinned digest. Neither route's answer, routing, rounding or position semantics moved.

Two findings stay open as drafts. 0072 records that the measured set is transversion-only, so the published figures cover the minority substitution class. 0073 records that a gene's score can depend on the other same-strand genes scored beside it, which is the cause of one of the two value disagreements, and that nothing a consumer reads says so.
