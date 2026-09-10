---
flow: build
priority: 4
---
# The repository says what a splice score is and is not evidence for

This software returns a number between zero and one for a variant, and says
nothing about what that number licenses a reader to conclude. Grepping every
file in the repository returns no hit for `ACMG`, `ClinGen`, `PP3`, `BP4` or any
calibrated threshold. A reader holding a score of 0.42 has no guidance in this
repository about what it supports, and the most likely thing they will do is
find a threshold published for a different predictor and apply it to this one.

That specific mistake is the one worth preventing. Thresholds calibrated for
another splice predictor do not transfer to this model, and the rank-equivalence
figure most often quoted when people attempt the transfer has been checked
against the published paper three separate ways and is not in it. Research
retained outside this repository records that check and its date, and forbids
citing the figure.

Settled: this repository publishes no threshold of its own and calibrates
nothing. It states what published, named authorities say, attributes each
statement to them, and says plainly where no calibrated guidance exists for this
model.

Settled: where evidence strength is described, the absence of calibration for
this model is stated as a limit on strength rather than left for the reader to
infer.

Done, observably:

- A reader learns that a score from this software has no published calibrated
  evidence threshold of its own, stated plainly rather than implied by silence.
- Any threshold or strength named is attributed to the body or publication that
  set it, carries its citation, and says which predictor it was calibrated for.
- The rank-equivalence figure that is not in the published paper appears
  nowhere, and the reason it is excluded is recorded where a later contributor
  will find it before re-adding it.
- A gate reads the citations out of the material, so a claim losing its
  attribution turns `make spec` red.
- `make test`, `make spec` and `make lint` pass.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag,
threshold or output. Nothing in this ticket causes the software to classify,
interpret or filter a variant, and no threshold enters the code. It publishes no
number this project measured or calibrated. It does not restate the response
shape and does not revisit what ticket 0056 settles. It names no private
consumer of this software.

This ticket is separable. If the project chooses not to speak about clinical
evidence at all, it is dropped whole rather than partly delivered, and the
material stays outside this repository.
