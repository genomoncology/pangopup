---
flow: build
priority: 2
---
# Five published claims are incomplete and nothing holds them

Five documented claims are each incomplete or wrong, and no gate reads any of
them, so each can go stale unnoticed. They are one ticket because they are one
job: correct a published sentence, then have a gate read it. Five claims, one
gate design.

## The stored version is placed on the status route only

Ticket 0052 put `data_set_version` on every HTTP score item. Two documents still
describe it as a status-route field, so a reader of either concludes a second
request is required. `architecture/service.md:37` opens "The status route also
publishes one `pangopup.scoring-data-set-version.v1` value as
`data_set_version`", while line 35 says of the other value "The status route and
every returned score item expose the same full SHA-256 value" -- the section now
tells a reader that one identity rides on the item and the other does not.
`README.md:133` says "Status and score items share the `scoring_identity`. Store
`data_set_version` as the data-set version when a system has one version field",
naming the value to store without saying the item carries it. Both sentences are
true and both are incomplete after 0052.

## The disagreement rate does not say which substitutions it covers

`architecture/compatibility.md` and `spec/score-value.md` publish a rate of
disagreement between the precomputed and modeled routes, measured over
`snv-stride-500k-v1`. That set holds 2,615 transversions and no transitions: its
rule ties the alternate base to the reference through a single four-step cycle,
and each step crosses the purine/pyrimidine boundary. Real traffic runs roughly
two transitions to every transversion, so the published figures cover the
minority class of what a consumer submits. Splice-site sequence is not
base-symmetric, so a transition-bearing set could move either figure in either
direction and nothing measured says by how much. The limitation is stated in the
artifact's `## Method`, two documents away from the figure it qualifies.

## Nothing says a score depends on the other genes scored beside it

`crates/pangopup-engine/src/lib.rs`, `score_typed`, walks the genes overlapping a
variant and calls `apply_mask` on shared `gain` and `loss` slices. The mask one
gene applies is still in place when the next same-strand gene is scored, so a
gene's answer depends on which other same-strand genes were scored before it.
The behaviour is upstream-faithful and the corpus pins it as
`P01-same-strand-order`. It is rare: a patched build computing both the shipped
cumulative answer and an isolated per-gene answer over every record of the
2,615-variant set found 3 records out of 3,038 where they differ, and one that
changes a score a consumer would read -- `chr19:51000000:G:T` /
`ENSG00000269741`, cumulative `gain 0.00` against isolated `gain 0.09 at -4`,
which is the published dataset's exact value. Nothing a consumer reads says any
of this.

## A published index size is computed, not measured

`architecture/index.md:75` states the complete corpus is 15,030,604,105 bytes.
The retained build artifact records the payload as 15,030,603,775 bytes at
`planning/artifacts/003-full-index-build.md:58`. The difference is 330 bytes,
exactly thirty loci at eleven bytes each: the published figure is 1,366,418,555
loci multiplied by eleven, and thirty of those loci are held in a separate
exception section rather than in the fixed-width payload. The document presents
an arithmetic product as a measurement, in a public document a reader may check
against a file they downloaded.

## Two published transcripts are no longer what the command prints

`mustmatch EXPECTED` compares canonical JSON when both sides parse as JSON, so
an added field fails the pin. `mustmatch like EXPECTED` compares a subset, so an
added field passes. Measured against mustmatch 0.1.0 on 2026-09-11:
`printf '{"a":1,"b":2}' | mustmatch '{"a":1}'` exits 1, and the same input under
`mustmatch like` exits 0.

`spec/` carries 24 JSON-object pins written with `like`. Rewriting every
`| mustmatch like '{` to `| mustmatch '{` and running `make spec` fails three of
them. `spec/snv-lookup.md:17` and `spec/model-routing.md:100` each pin a whole
lookup record whose `provenance` object does not carry
`"software_version":"0.5.0"`. The product emits it; it arrived in `d37d558` on
2026-09-09 and neither transcript changed. A reader takes those blocks as the
bytes the command prints, and they are not. The third, `spec/full-bundle.md:32`,
is a deliberate three-of-eleven-field excerpt beneath a paragraph about
determinism, and is correct as it stands.

This is the staleness ticket 0085 found in `spec/model-kernel.md`, where an
inspect transcript had been missing `"representation":"singleton"` for 47 days.
There the fence pinned nothing at all. Here the fence pins and still lets a
field arrive unnoticed.

Done, observably:

- `architecture/service.md` tells a reader that every returned score item
  carries `data_set_version`, in the same section and with the same force as the
  sentence that already says it about `scoring_identity`.
- `README.md` tells a consumer storing the data-set version that the score item
  already carries it, so no status request is needed to retain it beside a
  score.
- Every published disagreement figure carries, where a reader meets it, the fact
  that the measured set holds transversions only and no transitions, and that a
  transition-bearing set could move either figure by an amount nothing measured.
- The published contract states that a gene's score can depend on which other
  same-strand genes overlap the same variant, and names the one record in the
  measurement set where the dependence changes an answer a consumer would read.
- Every byte size published in the architecture folder is one that was measured;
  where a figure is derived by arithmetic the document says so and says what it
  is derived from; and the loci held outside the fixed-width payload are
  accounted for where the payload size is stated.
- None of the four can go stale unnoticed: a gate reads each claim out of the
  document that carries it, so deleting or reversing any one turns `make spec`
  red. The gate that already holds the disagreement figures to their artifact
  holds the substitution sentence too, and the same-strand statement is held to
  the frozen corpus case that pins the behaviour.
- Every `spec/` block presenting a whole record as the bytes a command prints
  pins them completely, so a field added to that record turns `make spec` red;
  a block deliberately pinning part of a record says in the file that it is
  partial and why.
- The gate counts the claims it read, so a scan matching nothing fails rather
  than passes.
- `make test`, `make spec` and `make lint` pass.

Boundary: this ticket changes documentation and the gates that pin it. It
changes no product source under `crates/`, does not change what any command
prints, does not alter `score_typed`, the
masking order, or any score, position, status, reason or provenance field. It
publishes no new number and re-measures nothing: the variant set, the artifact's
counts, the committed records, the mechanism statement and the ordering
statement all stand, and the certified installed member size of 15,033,158,255
bytes is correct wherever it appears and is not touched. It rebuilds no index
and changes no on-disk format. No document acquires a second, differently worded
contract: `architecture/compatibility.md` stays the one place the full response
shape is enumerated, and corrected sentences point there rather than restating
it. It does not move `data_set_version` onto or off any route, does not change
the sentences the existing spec gates already pin, and does not revisit what
tickets 0052 or 0059 shipped.

This repository does not tell a reader what a score is evidence for. It states
what the software computes and where the number came from. Interpretation,
evidence strength and clinical classification are outside what it speaks to.
It names no private consumer of this software.
