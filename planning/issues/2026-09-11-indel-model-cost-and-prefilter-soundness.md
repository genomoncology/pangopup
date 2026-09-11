# Indel model cost, and what a location prefilter may and may not skip

Status: open

## Observation

Every indel inside a gene runs the Pangolin model. The index holds
substitutions only, so an indel has no lookup route. Over a whole genome that
is a large, repeated cost, and the obvious way to cut it is unsound for half of
what the score reports.

The obvious cut is a location prefilter: skip indels that sit far from an
annotated exon boundary. `apply_mask` in
[`crates/pangopup-engine/src/lib.rs:1320`](../../crates/pangopup-engine/src/lib.rs)
shows why that cut is safe for one direction and destructive for the other.

The masking rule is asymmetric. Loss is clamped to zero at every window
position that is not an annotated boundary. Gain is clamped to zero only at the
annotated boundaries themselves. When no boundary falls inside the distance-50
window, loss is therefore exactly zero and gain is untouched.

Gain reports the creation of a new splice site. A new site appears away from
the existing ones. Skipping the positions far from a boundary skips exactly the
cryptic-site case a deep intronic workup is looking for.

## Why it matters

Scale, from the published per-genome variant counts and this repository's own
corpus counts.

A typical genome carries 546,000 to 625,000 indels
([1000 Genomes Phase 3](https://pmc.ncbi.nlm.nih.gov/articles/PMC4750478/)).
The index covers 1,366,418,555 gene loci
([`planning/artifacts/005-snv-transport.md`](../artifacts/005-snv-transport.md)),
which is 46.3 percent of the 2,948,318,359 ungapped bases of GRCh38.p14
(`GCF_000001405.40`). Assuming indels distribute in proportion to sequence,
250,000 to 290,000 of them land in a gene body and reach the model.

At the retained 4.3-second uncached inference that is roughly thirteen days of
single-core CPU for one genome. The SQLite cache removes the repeat cost and
none of the first-pass cost.

The same arithmetic on substitutions is the argument for the index rather than
against it: 1.6 to 2.0 million genic SNVs answer from the index in under one
second in total, and would take about ninety days through the model.

## What the code and the artifacts already establish

**The coarse filter already ships.** The mask query runs before any inference,
and a variant in no gene is rejected without one
([`crates/pangopup-engine/src/lib.rs:907-909`](../../crates/pangopup-engine/src/lib.rs)).
That already removes the roughly 290,000 to 335,000 indels per genome that fall
outside every gene. The mask member is 6,703,320 bytes and answers at 171 ns
p50 and 331 ns p95
([`planning/artifacts/012-gencode-mask-format-selection.md`](../artifacts/012-gencode-mask-format-selection.md)).

**The exon boundaries are already in hand at the right moment.** `MaskGene`
carries its `boundaries` when `self.mask.query` returns at line 907, before the
kernel runs. Today those boundaries are read only afterwards, inside
`apply_mask`. Deciding "is any boundary inside this window" needs no new data
and no new lookup.

**The corpus is overwhelmingly zero.** 310,269,258 of the 1,366,418,555 loci
carry any non-default score/position pair, and 549,194,849 of the 8.2 billion
possible pairs are non-default
([`architecture/index.md`](../../architecture/index.md)). Ticket 0059 measured
the same shape from the other side: 2,409 of 2,790 compared genic records score
zero on both routes
([`planning/artifacts/0059-route-disagreement-rate.md`](../artifacts/0059-route-disagreement-rate.md)).

## Three candidate directions

**A. Loss-only prefilter.** Exact, not approximate. `apply_mask` proves loss is
zero when no boundary falls in the window, so a caller that only wants
loss-of-site evidence can skip those calls and receive the same answer. Cheap
to build because the boundary list is already resolved. Its cost is that it
produces a deliberately partial score, and the output must say so rather than
look like a complete one.

**B. Predict the indel from the substitution neighbourhood.** If all three
substitutions at a locus score zero, an indel there may also score zero. The
reason to doubt it is that an indel shifts the sequence register and can create
or destroy a motif that no single-base substitution at that position can. This
is a measurement, not an argument, and nothing here has measured it.

**C. Extend the index to a bounded indel class.** Illumina's precomputed
SpliceAI files cover "all possible substitutions, 1 base insertions, and 1-4
base deletions within genes", so the shape is known to be feasible. Those files
are CC BY-NC 4.0 and unusable here, and the Wagner/Neverov Zenodo deposit this
project consumes is substitutions only. Extending the index means computing the
values, which is roughly 2.7 times the substitution corpus in model
evaluations and returns the project to the GPU cost it avoided. Size grows in
the same proportion, so a 15 GB index becomes roughly 40 GB.

A and C are engineering choices with known costs. B is the one that could
remove most of the cost cheaply, and it is the one that is unmeasured.

## The measurement B requires

Ticket 0059 is the template. It named a variant set by a stated rule, ran both
routes over it, reported a disagreement rate, and wrote down what the rate does
not cover.

The equivalent here reports a false-negative rate: over real indels whose
surrounding substitutions all score zero, how often does the model return a
non-zero score anyway, and how large are those scores. A prefilter is only
worth shipping if that rate is both small and bounded by something stated.

Report gain and loss separately. The two directions have different masking
rules, so one rate over both hides the answer.

## Data to harvest

The test set must be real observed indels rather than synthetic ones, because
the question is about the indels a caller actually submits.

- **gnomAD v4 genomes sites VCF, GRCh38, per chromosome.** Real indels with
  allele frequencies, so the set can be stratified by rarity. This is the
  primary source for the "ordinary genome" rate.
- **ClinVar VCF, GRCh38 weekly release, NCBI.** Enriched for pathogenic
  splice-altering indels. This is the more important half: a prefilter that is
  accurate on average and wrong on pathogenic variants is worse than no
  prefilter. Stratify by clinical significance and by molecular consequence.
- **The repository's own corpus** supplies the substitution neighbourhood for
  every candidate position at no cost.

Stratify the sampled set by distance to the nearest annotated boundary, by
indel length, by insertion against deletion, and by strand. `apply_mask` treats
gain and loss differently by position, so a set that does not vary distance
cannot answer the question.

Before any of this is downloaded, confirm and record the terms of use for each
source. This repository exists because of a licensing constraint, and a test
corpus with the wrong terms is not usable evidence. gnomAD and ClinVar are both
expected to be usable, and neither was verified while writing this note.

## The oracle, and a licensing trap

The oracle is Pangolin itself, reached through this project's own model route.
The question is whether the substitution neighbourhood predicts what the model
returns, so the model is the ground truth and no external scores are needed.

SpliceAI's precomputed scores must not be used as the oracle. They are a
different model, and they are CC BY-NC 4.0. Using them to qualify a shipped
filter would put a non-commercial dependency underneath the product, which is
the exact outcome this project was built to avoid.

## What a ticket would need before it is ready

Per [`planning/README.md`](../README.md), a slice is selected only once its
input evidence, observable acceptance test, and failure tests are known. None
of those is known yet, so this stays an issue.

- The named variant set and the rule that generates it, in the 0059 style, with
  its manifest retained.
- The confirmed terms of use for every harvested source.
- The measured accelerator throughput from
  [`2026-09-11-model-throughput-on-accelerators.md`](2026-09-11-model-throughput-on-accelerators.md).
  Direction C and any precompute scope are uncostable without it.
- The stated acceptance threshold: what false-negative rate, measured over
  which strata, would make a prefilter shippable, decided before the number is
  seen.
- Whether the deliverable is a filter inside PangoPup or a documented rule for
  callers. Option A argues for a flag on the request. Option B, if it measures
  well, argues for a route decision the engine makes on its own, and that
  changes the output contract because a skipped call still has to report that
  it was skipped and why.
- What happens on the unsupported edges `apply_mask` does not cover: genes with
  an empty boundary list already emit `no_annotated_sites`, and that path is
  distinct from "boundaries exist but none in this window".
