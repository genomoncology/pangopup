---
flow: build
priority: 3
---
# The published disagreement rate says which substitutions it covers


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

The number stands. A second measurement over a transition-bearing set is not
this ticket: the routes differ by a reduction rule that the 0059 investigation
showed is deterministic, and no evidence says a transition would exercise it
differently. What is missing is that a reader of the published figures is not
told which substitutions they were measured over. `architecture/compatibility.md`
names one substitution class only in the ordering sentence, and
`spec/score-value.md` never names it at all. The limitation lives in the
artifact's `## Method`, two documents away from the figure it qualifies.

Done, observably:

- Every published disagreement figure carries, in the same place a reader meets
  it, the fact that the measured set holds transversions only and no
  transitions.
- A reader is told plainly that a transition-bearing set could move either
  figure and that nothing measured says by how much.
- The statement cannot go stale: the gate that already holds the figures to the
  artifact holds this sentence to it too.
- `make test`, `make spec` and `make lint` pass.

Boundary: this ticket publishes no new number and re-measures nothing. It does
not change the variant set, the artifact's counts, the committed records, the
mechanism statement or the ordering statement. It changes no product source
under `crates/`, no score, position, status, reason or provenance, and it does
not revisit what ticket 0059 shipped.
