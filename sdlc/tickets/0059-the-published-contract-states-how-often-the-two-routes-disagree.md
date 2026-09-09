---
flow: build
priority: 1
---
# The published contract states how often the two scoring routes disagree

`architecture/compatibility.md:60` tells a consumer that a precomputed score and a modeled score are not interchangeable, shows one disagreeing record, and then says "No rate of disagreement is claimed." The only evidence behind that warning is the frozen upstream corpus: four variants, five gene records, one disagreement. `GRCh38:chr10:114306065:A:T` reports `0.06` at position 12 from the published dataset and `0.02` at position 13 from the model.

A consumer deciding whether to mix the routes, or to accept a modeled answer where a precomputed one is missing, has nothing to judge the risk with. One record out of five supports no number at all, and a reader who divides them gets 20 percent, which we have not measured and do not claim.

The measurement runs before v0.5.0 is published, not after. The result can change what the release says. Publishing "no rate is claimed" and then discovering a bad rate means retracting a statement consumers have already stored scores against.

Settled: this ticket measures and publishes. It does not change routing. If the measured rate is high enough to change the guidance a consumer follows, this ticket records the number and files a successor rather than quietly rewriting the guidance.

Settled: value disagreement and position disagreement are reported separately. Line 60 already says positions diverge more widely than values, with no number behind it. A zero score carries no meaningful position on either route, so the measurement has to say how it treats those rather than averaging them in.

Done, observably:

- The repository states a measured disagreement rate between the precomputed and the modeled route, over a named variant set, recording the set, its size, and the date measured.
- The statement reports value disagreement and position disagreement as separate numbers, and says how zero scores were treated.
- `architecture/compatibility.md` no longer says no rate of disagreement is claimed.
- The evidence lives in a durable artifact a later reader can re-run to get the same number.
- A gate holds the published number to that artifact, so the two cannot drift apart.
- `make lint`, `make test` and `make spec` pass.

Boundary: this ticket changes what the repository states about the two routes and adds the evidence behind it. It must not change either route's answer, which route a request takes, or any score value, position, status, reason or provenance field.

It must not change the frozen upstream corpus, the oracles under `tests/fixtures/`, or their pinned digests.

It must not change the rounding contract, the position semantics, or the guidance about reading a position only where the score beside it is non-zero. Ticket 0040 settled those.
