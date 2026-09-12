# Can the SNV landscape safely screen indels?

Date: 2026-09-12

## Question

PangoPup already holds the Pangolin score for every supported SNV. Determine whether those scores can answer either of these questions without running the model for an insertion or deletion:

1. Can nearby SNV scores reliably place an indel above or below a caller-supplied threshold?
2. Do stretches of low or zero SNV scores identify regions where an indel can safely be ignored?

The earlier observed-indel recurrence work answers a different question. It measures exact indel reuse. It does not answer either question here. Keep that work as a secondary strategy.

## Decision

Measure the SNV landscape as an approximate indel screen before doing more catalogue work. Start with simple rules that a caller can inspect. Do not train another model unless the simple rules leave useful signal that they cannot express.

This decision supersedes the catalogue-first next step in `planning/artifacts/2026-09-12-indel-fast-path-pilot.md`. Exact indel recurrence remains a secondary strategy. Catalogue membership alone is not an available fast score because the catalogue still needs PangoPup results.

The accepted cost is a model-labelled research corpus. Empirical testing can estimate an error rate. It cannot turn an SNV proxy into an exact biological guarantee.

Ian can overturn this priority. The measurement itself changes no public API or score.

## Inputs already present

- The exact 15,033,158,255-byte production SNV index is on the Mac.
- The production Pangolin model, GRCh38 reference, GENCODE mask, and runtime profile are on the Mac.
- Public native-GRCh38 1000 Genomes genotype VCFs are on `yellow.local`. They provide real insertions and deletions without requiring another download for the pilot.
- The prior measurement already extracts supported genic indels while preserving the submitted chromosome, position, reference sequence, and alternate sequence.

Yellow selects and aggregates variants. The Mac performs SNV lookups and model scoring. Retained public artifacts contain no person identifiers.

## Scope of the first study

The first study tests below-threshold screening. It asks whether a fast rule can say that the complete current PangoPup indel result stays below a caller-supplied gain or loss threshold. It does not infer pathogenicity. It does not assign clinical ranks. It does not claim that a high nearby SNV score proves a high indel score.

An above-threshold or multi-rank study remains separate. Start it only if the below-threshold study finds useful signal. That study needs independently defined rules and full confusion matrices.

## Paired observation

Score each distinct literal indel once. Create one analytical row for each indel, returned gene, and score direction. The row contains the full current PangoPup model result plus summaries of the published SNV scores around the changed span for the explicitly matched stable gene. Pin the runtime profile, SNV bundle, model, reference, mask, software version, and CPU policy.

Preserve and validate the literal input against the pinned reference. Use 1-based closed reference intervals. An anchored insertion `POS, REF, ALT` has changed span `[POS, POS]`. An anchored deletion has changed span `[POS, POS + len(REF) - 1]`. Expand each side by 10, 25, 50, or 100 bases and clip at the contig boundary.

Inspect the anchor alone as a baseline, then inspect all three possible non-reference SNVs at each reference base in the larger windows. Match a published record to the model row through the exact stable Ensembl gene identifier. A missing record, ambiguous source reference, or gene mismatch forces model fallback for the initial rules. Never convert it to zero. The inserted sequence remains its own feature because no SNV can represent bases that do not exist in the reference.

For each window and score direction, retain at least the maximum public score magnitude, count of non-zero public scores, fraction of public scores equal to zero, distance from the changed span to the nearest SNV locus with a non-zero score, and whether every expected record was present. Published values use hundredths. Loss comparisons use the magnitude of the signed public loss value. Also retain insertion versus deletion, changed length, inserted sequence, distance to the nearest annotated exon boundary, strand count, and number of overlapping genes.

## Rules to test

Test transparent below-threshold rules first. A complete rule definition names the window, maximum permitted SNV score, target indel threshold, score direction, eligible insertion or deletion lengths, and missing-data fallback:

- Every surrounding SNV score is exactly zero.
- The largest surrounding SNV score is at most 0.00, 0.05, 0.10, 0.20, or 0.50.
- The SNV rule plus distance from the nearest annotated exon boundary.
- The proposed exact loss threshold bound. After successful request admission, no negative loss contribution can survive when the deletion-expanded output span contains no relevant boundary under the current gene-specific masking semantics. This can answer a positive loss-threshold question as below threshold. It does not supply a complete modeled result or a meaningful score position.

Evaluate gain and loss separately. The exact loss rule says nothing about gain. Evaluate insertions and deletions separately and split results by changed length.

Use 0.10, 0.20, and 0.50 as caller-supplied performance scenarios. They are measurement points, not clinical defaults.

## Representative traffic and challenge cases

Keep two datasets separate:

- A probability sample from real person-level traffic estimates fast-path coverage and error rates. Record each selection probability and weight the result back to traffic. Retain every outcome from the sample, including zeros, unsupported requests, failures, and missing records.
- An enriched challenge set supplies additional threshold-crossing events across score direction, indel kind, length, exon-boundary distance, strand count, and overlapping-gene count. It measures failure modes. It does not estimate traffic coverage.

Score each distinct indel once, then join the result back to each person-level occurrence for traffic estimates. Calculate complete-request resolution separately from analytical gene rows. A request remains unresolved when any requested gene or score direction remains unknown.

## Evaluation

Use whole chromosomes as development, validation, and final test partitions. Freeze the rules before the final test. If a later study uses genomic blocks, exclude a buffer that covers both the largest SNV feature window and the model context. Keep relatives together. Do not select an operating point from the final test curves.

For every rule and threshold report:

- The weighted percentage of complete indel requests resolved by the fast rule.
- The weighted percentage of full-model threshold-crossing requests that the rule incorrectly dismisses.
- The percentage of fast dismissals that are wrong.
- Per-gene and per-direction disagreement separately from complete-request resolution.
- Unique-indel counts separately from weighted person-level traffic.
- Counts behind every percentage and uncertainty calculated by resampling whole people or families and genomic blocks. Do not treat repeated alleles, overlapping windows, or multiple gene rows as independent binomial trials.
- Separate results for gain, loss, insertions, deletions, length groups, exon-boundary distance groups, chromosomes, and populations when available.
- The number of SNV queries, cold and warm lookup cost, missing-record fallbacks, model calls avoided, residual model cost, and projected whole-genome elapsed time.
- The additional complete requests resolved by SNV evidence beyond ordinary routing and the exact loss baseline on the same requests.

A zero observed disagreement does not mean zero risk. Report the statistical upper bound. For the engineering pilot, seek 500 threshold-crossing unique indels from distinct non-overlapping 10-kilobase blocks for each primary direction and insertion-or-deletion stratum. Select at most one event per block for the zero-error bound. Use an exact one-sided 95 percent Clopper-Pearson interval. Zero errors among 500 independent selected blocks has an upper bound of about 0.6 percent. Stop at the target or exhaustion of chromosome 22 and label any smaller stratum inconclusive. Repeated occurrences supply traffic weight and never increase the accuracy sample size.

For final confirmation, seek 3,000 threshold-crossing unique indels from distinct blocks for each claimed primary stratum. Zero errors among 3,000 has an exact one-sided 95 percent upper bound of about 0.1 percent. A non-zero error analysis must report the same exact interval plus a block-cluster sensitivity analysis. These are research expansion targets for agreement with PangoPup. They are not clinical acceptance thresholds. Do not choose an acceptable clinical error rate without a source. Publish the coverage-versus-disagreement curve so callers can choose a supported operating point.

## Staged execution

1. Build and test the paired-row extractor against miniature fixtures. Prove correct gene matching, all alternate SNVs, missing-record handling, insertion and deletion spans, windows, both score directions, overlapping genes, and threshold boundaries.
2. Measure how often anchor-only and windowed SNV inputs are complete and how often zero or low-score windows occur before paying for every model label.
3. Run a chromosome 22 engineering pilot. Keep a weighted representative sample separate from an enriched challenge set across insertions, deletions, lengths, and exon-boundary distances. Include an early sequence challenge at apparently quiet loci across both strands and several boundary distances. Test all four single-base insertions and anchored deletions removing one through four reference bases. A counterexample defeats an unconditional claim for its tested class. A passing challenge supports only the tested loci, sequences, and lengths. Use the pilot to verify the pipeline and reject clearly useless rules. Do not call chromosome 22 a held-out confirmation.
4. Let Astra review the method and pilot result before expanding it.
5. Compare gain-only rules against current empty-cache indel routing. Compare loss-only rules against the proposed exact annotation-bound route after its equivalence checks pass. Compare complete gain-and-loss rules against current empty-cache routing because a partial loss answer does not resolve the complete request. Do not use an unscored population catalogue as a baseline.
6. Expand to development and validation chromosomes when a pilot rule resolves at least 10 percent of the complete requests that its named baseline would send to the model, meets the pilot event target without an observed wrong dismissal, and produces measured runtime savings. This is an experiment-expansion rule. It is not a clinical acceptance threshold or a production gate.
7. Freeze candidate rules and run them once on final test chromosomes. Any rule selected after viewing that result requires another untouched test set.
8. If a simple rule survives final testing, extend the early sequence challenge with unseen supported inserted sequences and deletion lengths before making a region-level claim. Observed cohort alleles cannot establish that every possible indel in a region behaves the same way.
9. Derive genomic intervals only after the sequence challenge. Preserve every condition on gene, score direction, caller threshold, indel kind, length, changed span, sequence, and missingness. A locus-only interval cannot replace those conditions.

## Interpretation

The result can support three outcomes:

- A measured rule resolves many indels with a supported disagreement range. Consider a threshold API backed by that approximate rule and preserve model fallback.
- A measured rule resolves only a small fraction. Keep the exact loss rule and exact precomputed-indel lookup. Do not add the tested SNV-based rule.
- The tested rules show no useful separation on this corpus. Record that result without claiming that every possible SNV-based method must fail. Model inference or exact precomputed indel scores remain necessary unless later evidence supports another route.

Success means agreement with the current PangoPup model inside the measured domain. It does not prove biological harmlessness or clinical safety. Published zero SNV scores are rounded and masked summaries. They do not prove that an indel leaves splicing unchanged.
