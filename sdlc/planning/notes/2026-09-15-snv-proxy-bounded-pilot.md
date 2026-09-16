# Bounded SNV-to-indel pilot

Date: 2026-09-15

## Decision

Run three small measurements before any full-genome study. The measurements test whether the published SNV landscape contains a plausible indel-screening signal. They do not qualify a production shortcut or clinical threshold.

The accepted cost is a small set of full-model indel evaluations. Ian can overturn the experiment order. The public API and assets remain unchanged.

## Experiment 1: Input availability

Use supported, anchor-genic, literal chromosome 22 indels from the existing public 1000 Genomes aggregate. Query the published SNV index for the anchor and a ten-base flank on both sides. Count complete gene-matched SNV neighborhoods, missing records, ambiguous records, all-zero gain neighborhoods, and all-zero loss neighborhoods. Keep insertion and deletion counts separate. This is a feasibility census, not an accuracy result.

## Experiment 2: Paired accuracy check

Score a fixed selection of real chromosome 22 indels with the current PangoPup model. Keep a hash-ranked observed-allele selection separate from deliberately selected quiet and non-quiet neighborhoods. Compare the full-model gain and loss magnitudes with nearby published SNV maxima at caller-supplied 0.10, 0.20, and 0.50 measurement thresholds. Count distinct indels and carrier-weighted occurrences separately. Never count a missing SNV as zero. Report each disagreement and the full denominator. This check can find counterexamples; its small sample cannot certify a low false-negative rate. The balanced allele sample and its carrier weights do not estimate patient traffic.

## Experiment 3: Quiet-position challenge

At a few quiet loci from Experiment 1, score alternative supported insertions and deletions that the population aggregate did not choose. Preserve literal anchor and reference validation. Record whether an indel crosses a measurement threshold despite all queried published SNVs scoring zero. A single such case rejects an unconditional dead-zone claim for that locus and indel class. No finite passing panel proves that every possible indel is harmless.

## Limits and stop rule

Use the pinned runtime profile and published bundle. Do not normalize literal alleles. Do not use the observed-indel catalogue as evidence for the SNV proxy. Do not interpret model agreement as biological or clinical safety.

Stop after the declared chromosome 22 pilot. If all-zero neighborhoods are scarce or incomplete, report the ceiling on this simple shortcut and do not score a larger corpus. If a quiet-neighborhood counterexample appears, report the rule as unsafe for blanket dismissal and do not promote it. If the small pilot shows useful separation, specify a larger independent chromosome study with its sampling and uncertainty method before treating any percentage as achievable.

Retain a concise report with exact inputs, hashes, counts, code revision, and failures. Keep person identifiers out of the repo.
