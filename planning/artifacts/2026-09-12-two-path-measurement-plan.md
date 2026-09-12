# Two-path indel measurement plan

Date: 2026-09-12

Pilot result: [`2026-09-12-indel-fast-path-pilot.md`](2026-09-12-indel-fast-path-pilot.md). The independent chromosome 22 result supports an observed gnomAD catalogue plus an exact loss-only route. It does not support a 1000 Genomes catalogue alone or a classifier before the exact routes are exhausted.

Related issues:

- `planning/issues/2026-09-11-indel-model-cost-and-prefilter-soundness.md`
- `planning/issues/2026-09-11-triage-which-variants-need-the-model.md`

## Objective

Determine whether an exact fast path can answer enough indel threshold requests to reduce measured whole-genome CPU time materially. The experiment must measure real routing and elapsed time. Catalogue size or genomic coverage cannot substitute for patient traffic.

This plan does not choose clinical thresholds, implement an API, or change the release plan.

## Requested answers

Measure these contracts separately because they have different resolution rates:

1. Gain-only threshold result for each returned gene.
2. Loss-only threshold result for each returned gene.
3. Complete gain-and-loss threshold result for each returned gene.
4. Separate gain and loss ranks for each returned gene.

Test 0.1, 0.2, and 0.5 as caller-supplied performance scenarios. They are not PangoPup clinical defaults. For a threshold `T`, an exact fast result requires either a stored public score or a proven public-score interval wholly below or wholly at or above `T`. A partial loss answer does not resolve a gain-and-loss request.

Preserve missing records, rejections, warnings, score direction, magnitude, position, and gene identity. Never convert missing or rejected data to zero.

## Statistical units and denominators

Retain separate counts for:

- Input VCF records.
- Submitted alternate alleles.
- Alternate alleles carried by each person.
- Unique PangoPup requests after the selected input preparation.
- Supported requests.
- Requests overlapping an in-scope gene by the declared overlap rule.
- Fully resolved requested answers.

Rank catalogue entries by the number of training people who carry the variant. Do not use allele count as patient traffic because a homozygous person contributes two alleles but one submitted variant.

Keep relatives in the same split. Report population-specific results. Weight any stratified sample back to person-level traffic and calculate uncertainty across independent people or families.

## Data design

### Exploratory frequency data

Use population sites data to measure possible catalogue size, indel length, gene overlap, and frequency concentration. Treat any all-person sites file as exploratory. It cannot measure held-out coverage because its allele counts already include every cohort member.

### Primary traffic data

Use a public, native-GRCh38 population genotype callset. Split people before building any catalogue:

1. Group relatives.
2. Assign groups to training and held-out sets.
3. Build and rank each measured catalogue from training genotypes only.
4. Measure exact coverage in held-out people.
5. Repeat with population holdouts.

### Secondary traffic data

An independent diagnostic-style cohort may test transfer after its data owner authorizes this use. It cannot replace the public primary measurement. Do not put private cohort names, identifiers, or storage paths in the public result.

## Variant identity

PangoPup scores the submitted position and alleles. Representation affects the context, output window, gene query, and cache identity.

Preserve both the original tuple and any prepared tuple. Apply the same declared preparation to training catalogue entries and held-out traffic, then score that resulting tuple. Report literal-input coverage separately when callers may submit unnormalized variants.

Count reference mismatches, symbolic alleles, unsupported bases, oversize alleles, multi-allelic decomposition, and other rejections. Do not silently discard them.

## Ordered fast-path waterfall

Assign each request to the first route that fully answers its requested contract:

1. Existing exact SNV route when applicable.
2. Exact indel score in the precomputed catalogue.
3. Exact persistent-cache result.
4. Exact gene-scope rejection under the declared span rule.
5. Exact loss mask or reference-probability bound.
6. Full Pangolin inference.

Report potential catalogue membership separately from score-backed catalogue coverage. Population files do not contain Pangolin indel results. A variant becomes a fast exact hit only after the matching submitted tuple has a stored Pangolin result.

Do not count a nearby-SNV rule as exact. Insertions add sequence. Deletions join separated sequence and change spacing. Any SNV-neighborhood rule is an approximation and requires its own error budget.

## Exact-bound qualification

An exact bound must reproduce the current public scoring semantics:

- Ties-to-even rounding to hundredths.
- Threshold comparison after public rounding.
- Deletion-expanded output windows.
- Per-gene boundary masks.
- Current gene order and cumulative array mutation.
- Both strands and overlapping genes.
- Empty boundary lists.
- Reference failures and all rejection outcomes.

A raw upper bound of 0.099 does not prove a public score below 0.10 because public rounding may produce 0.10.

Build an executable equivalence suite around every rounding boundary and structural case. Exact fast decisions require zero disagreements with the existing full-model threshold result.

Reference precomputation needs a qualification experiment before a whole-genome build. Prove that tiling, context length, strand, ensemble handling, and boundary extraction preserve every claimed bound.

## Runtime experiment

Run the baseline and proposed waterfall on the same held-out requests, host, runtime profile, and concurrency. Record cache state. Include parsing, routing, lookup, inference, serialization, and output writing.

Measure:

- Fully resolved percentage for each requested contract and threshold.
- Model evaluations avoided.
- End-to-end elapsed time.
- Mean, median, tail, and maximum request latency.
- Miss cost by insertion or deletion, allele length, strand count, location, and threshold.
- Catalogue bytes and build cost.
- Reference-precompute bytes and build cost.
- Number of whole-genome workloads required to repay each precompute cost.

Do not apply one 4.3-second example to every residual miss. The fast path may leave a slower request mix. Measure the retained misses directly.

## Interpretation

Use measured elapsed time as the decision variable. Resolution rate provides the explanation.

- Less than 90% exact fast resolution cannot serve as the primary performance solution under the current miss cost.
- At least 90% supports a useful secondary path but still leaves hours of illustrative whole-genome work.
- Roughly 99% is the region associated with an illustrative one-hour eight-core genome.
- Roughly 99.8% is the region associated with a 10-millisecond average indel response when a fast result costs 1 millisecond and a miss costs 4.3 seconds.

Replace these arithmetic projections with measured whole-genome results before an implementation decision.

## Independent review

Reviewer: Astra, Extra High

Disposition: accepted as an exploratory measurement plan after revision.

The review required these changes:

- Prevent catalogue leakage by rebuilding rankings from training genotypes.
- Define complete requested answers and report gain-only, loss-only, and combined coverage separately.
- Preserve literal and prepared variant identities.
- Distinguish population membership from a score-backed fast hit.
- Measure the residual miss distribution and end-to-end runtime.
- Count carried variants per person and group relatives during splitting.
- Make rounding, deletion, gene-order, masking, strand, and rejection equivalence executable acceptance criteria.
- Keep private inventory details outside the public repository.
