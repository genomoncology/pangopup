---
flow: build
priority: 3
---
# The repository says why it exists and what a consumer may do with it

A reader arriving at this repository learns what the software does and never
learns why it exists or whether they are allowed to use it. `README.md:3` says
"GPL-licensed" and `README.md:245-247` names GPL-3.0-only and the NOTICE file.
Nothing anywhere says what that licence means for someone whose own software is
not GPL. The repository has no hit for `separate program`, `mere aggregation` or
`own license` in any file.

The motivation is absent in the same way. Grepping every Markdown file returns
no hit for `Illumina`, `commercial`, `non-commercial`, `rate limit` or
`spliceailookup`. `SpliceAI` appears twice, both in
`sdlc/issues/2026-09-04-splice-consequence-module-research.md:14,44`, as future
research rather than as a reason this project was built. A reader cannot tell
why a precomputed Pangolin index exists when other splice predictors are
better known.

`architecture/README.md:3-28` is the third case. It opens with the project's own
build history, written for the agents that produced it: it names ticket numbers,
an ineligible batching run, a v2 exporter's omitted dynamic axes, and retained
host-qualified channel mappings. Someone opening the architecture folder to
learn how the system is arranged learns the order the work happened in instead.

Research retained outside this repository has already gathered the primary
sources for the first two, with verbatim captures and capture dates: the
upstream predictor's own licence text, the public lookup service's own stated
terms, and the upstream model repository's packaging. Those sources are what
must be quoted. This repository states no legal conclusion of its own.

Settled: the licence explanation describes the arrangement in engineering terms,
quotes or links the licence text that supports it, and says plainly that it is
not legal advice. It does not assert what a court would decide.

Settled: a consumer of this software is described generically. No product name,
no repository name, no branch or ticket key, no commit identifier and no
individual is named.

Done, observably:

- A reader who opens `README.md` and follows one link learns why this project
  exists, with each claim about another party's licence or service carried as a
  dated quotation attributed to that party rather than as this project's
  assertion.
- A reader learns what calling this software from other software means for that
  software's own licence, and reads a plain statement that the explanation is
  not legal advice.
- The architecture folder opens with a description of how the system is
  arranged, in plain sentences, above whatever build history is kept.
- Every external claim carries the source it came from and the date that source
  was read, so a later reader can check whether it still holds.
- A gate reads the sourcing requirement out of the new material, so removing a
  citation or its date turns `make spec` red.
- `make test`, `make spec` and `make lint` pass.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag or
output. It publishes no new measurement and re-measures nothing. It does not
restate the response shape: `architecture/compatibility.md` remains the one
place the full shape is enumerated, and this ticket does not revisit what ticket
0056 settles about where a stored version is stated. It does not correct the
index payload figure, which is ticket 0079. It names no private consumer of this
software under any circumstances.

This repository does not tell a reader what a score is evidence for. It says
what the software computes and where the number came from. Interpretation,
evidence strength and clinical classification are outside what this repository
speaks to, and this ticket does not open that door.
