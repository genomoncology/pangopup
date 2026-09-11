# Deciding cheaply which variants are worth the model

Status: open

## The question under the other four issues

The index exists so that a covered variant never runs the model. Everything not
covered runs the model at about 4.3 seconds. The useful question is not "how do
we precompute everything", because that is not reachable. It is "how do we tell,
cheaply, that a variant cannot matter".

A variant that cannot matter needs no score at all. A variant that might matter
needs the real model. A cheap decision between those two is worth more than a
larger index, because it bounds the work rather than enlarging the table.

## Exhaustive indel precompute is not reachable

A deletion at a position is determined by its length. An insertion is a
sequence, so insertions of length L give 4^L possibilities at every position.

Over the 1,366,418,555 gene loci in the corpus:

| Indel class | Records per locus | Total records |
|---|---:|---:|
| Current SNV corpus | 3 | 4,099,255,665 |
| All indels up to 1 bp | 5 | 6,832,092,775 |
| SpliceAI's class: 1 bp insertions, 1-4 bp deletions | 8 | 10,931,348,440 |
| All indels up to 2 bp | 22 | 30,061,208,210 |
| All indels up to 4 bp | 344 | 470,047,982,920 |
| All indels up to 10 bp | 1,398,110 | 1,910,403,445,931,050 |

Enumeration dies between 2 and 4 bases. There is no version of this that covers
indels in general.

## Coverage of the possible space is the wrong measure

Ian raised the objection directly: if the reachable precompute covers a
vanishing fraction of possible indels, why build it.

The answer is that a cache is judged on the traffic it serves, not on the space
it spans. Two fractions matter and neither has been measured here:

1. **Length concentration.** What fraction of indels observed in gnomAD inside
   gene bodies falls within a bounded class such as 1 bp insertions and 1-4 bp
   deletions. Real indels concentrate sharply at short lengths, so this fraction
   is expected to be large while the space fraction is near zero.
2. **Recurrence.** What fraction of the indels in a newly sequenced genome has
   already been observed in gnomAD or ClinVar. For substitutions this fraction
   is high. For indels it is lower and has not been quantified here.

The second fraction is the hit rate of an observed-only index, and it is the
number that decides whether precomputing observed indels is worth doing at all.

Both are extractable from the gnomAD and ClinVar VCFs named in
[`2026-09-11-indel-model-cost-and-prefilter-soundness.md`](2026-09-11-indel-model-cost-and-prefilter-soundness.md)
with counting alone. Neither needs a GPU, a model run, or a format change. They
should be measured before any precompute scope is chosen.

## A cheap model that only says yes or no

The remaining traffic is variants no table will hold. The proposal is a small,
fast classifier that answers one question: is this variant certainly below the
decision threshold.

The error budget is one-sided. Saying "run the model" about a harmless variant
costs 4.3 seconds once. Saying "ignore" about a real splice variant is a missed
diagnosis. A useful classifier therefore targets near-zero false negatives and
accepts a high false-positive rate, and it is measured by how much traffic it
removes at a fixed false-negative rate, not by accuracy.

This is the same structure as speculative decoding: a cheap draft proposes, an
expensive model verifies, and only the uncertain cases are verified.

Three properties make it plausible here.

**The training labels already exist.** The corpus is 4,099,255,665 labelled
substitution records. That is an unusually large supervised set, and it is
already on disk.

**The strongest features are already computed.** Distance to the nearest
annotated boundary comes from the GENCODE mask at 171 ns
([`planning/artifacts/012-gencode-mask-format-selection.md`](../artifacts/012-gencode-mask-format-selection.md)).
The substitution scores surrounding any locus come from the index for free. That
second feature is direction B of the indel issue, generalized.

**The masking rule already proves one exact case.** `apply_mask` at
[`crates/pangopup-engine/src/lib.rs:1320`](../../crates/pangopup-engine/src/lib.rs)
makes loss exactly zero when no annotated boundary falls inside the distance-50
window. That is not a prediction, so a loss-only caller gets a free exact
filter. Gain gets no such rule, which is why the classifier is needed for the
rest.

Indel labels do not exist yet. Producing them is what the accelerator run in
[`2026-09-11-model-throughput-on-accelerators.md`](2026-09-11-model-throughput-on-accelerators.md)
would be for. The training set is the deliverable of that run, whether or not
the scores ever ship as an index.

## The threshold this all hangs on

A triage classifier needs a threshold to triage against, and every design here
assumes one exists.

Ian cited 0.106. Searching found that number attributed to Pangolin in
comparative work, described as chosen so that an equal number of predictions
pass the Pangolin cutoff and the SpliceAI 0.1 cutoff. If that is correct, 0.106
is a rank-matching device for comparing two tools and not an independently
calibrated clinical threshold.

**The primary source was not read.** The Pangolin paper is at
[doi:10.1186/s13059-022-02664-4](https://doi.org/10.1186/s13059-022-02664-4) and
bioRxiv 2021.07.06.451243. Springer redirected to an authorization endpoint and
bioRxiv returned 429. The claim above rests on search summaries and must not be
built on until someone reads the paper.

The separate and more relevant body of work is the ClinGen SVI Splicing
Subgroup's calibration of splice predictions to ACMG evidence strengths
([Walker et al., AJHG 2023](https://www.sciencedirect.com/science/article/pii/S0002929723002033)).
That work assigns evidence strength by likelihood ratio across score bands
rather than by a single cutoff, and it reports that SpliceAI at 0.5 may be
calibrated too high. A banded evidence model changes the triage design, because
the classifier would separate bands rather than cross one line.

This is the piece to settle first. The size, precision and throughput issues are
all engineering with known measurements. This one determines what the engineering
is aiming at, and it is currently an unverified number.

## What a ticket would need

- The primary-source reading of where 0.106 comes from and what it means.
- A decision on whether PangoPup expresses a single threshold, the ClinGen
  bands, or neither, recorded as an ADR. Expressing neither and leaving the
  threshold to the caller is the reversible option.
- The two measured fractions above: length concentration and recurrence.
- The stated false-negative budget for any classifier, decided before a model is
  trained.
- Whether triage is a PangoPup feature or a documented rule for callers. A
  shipped classifier changes the output contract, because a skipped variant must
  report that it was skipped and why.
