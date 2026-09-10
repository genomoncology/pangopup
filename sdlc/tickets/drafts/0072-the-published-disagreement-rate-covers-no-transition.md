---
---
# The published disagreement rate covers no transition

`architecture/compatibility.md` and `spec/score-value.md` now publish a rate of
disagreement between the precomputed and the modeled route, measured over
`snv-stride-500k-v1` and recorded in
`planning/artifacts/0059-route-disagreement-rate.md`.

That set holds 2,615 transversions and no transitions. Its rule ties the
alternate base to the reference base through the single cycle `A→C→G→T→A`, and
each of those four steps crosses the purine/pyrimidine boundary, so no
transition can be drawn. Real variant traffic runs roughly two transitions to
every transversion, so the published figures cover the minority class of the
substitutions a consumer will actually submit.

Splice-site sequence is not base-symmetric. A set that includes transitions
could move either figure in either direction, and nothing measured says by how
much. The artifact's `## Method` now states the limitation, but the number
itself still rests on one substitution class.

A successor either draws a second set whose alternate base is chosen so that
both classes appear, re-measures, and publishes the two figures per class, or
records a reasoned decision that one class is enough and says why.
