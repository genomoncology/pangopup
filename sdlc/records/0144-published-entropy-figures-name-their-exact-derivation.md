# Published entropy figures name their exact derivation

The entropy section now traces both retained byte totals to the checked analyzer and report. The separate-stream total uses one pooled complete-record entropy across 4,099,255,665 SNV rows plus reference entropy across 1,366,418,555 loci, with the sum divided by eight. The joint total uses complete reference-plus-three-alternate locus entropy across those loci, divided by eight. The analyzer calculates with unrounded `f64` entropy values, displays entropy to six decimal places, and displays bytes to the nearest whole byte. The rounded values therefore do not reproduce the exact totals.

The implementation did not run the analyzer, read the source corpus, or change an index decision. It read the retained Rust calculation and made the existing evidence explicit. A focused claim check reads the retained report and analyzer.

Code review found that the first checker could accept negated precision, let one calculation borrow a term or divisor from another paragraph, and accept a wrong prose total when the code block retained the right number. The repaired checker requires affirmative unrounded precision for both totals, refuses displayed or rounded joint-locus input, keeps each formula's inputs and divisor in its own paragraph, and inventories separate prose and fenced-code occurrences. Its mutations reject all five review counterexamples, three distinct alternate-record entropies, missing joint terms, drift in either total, wrong display precision, and missing source attribution. A paragraph reflow passes.

The new check failed against the prior documentation because the entropy section did not publish the retained row count. It passed after the documentation repair. `make lint`, `make test`, `make spec`, and `git diff --check` passed on macOS. The specification gate ran 207 blocks.

Draft 0104 and Ticket 0144 are archived with this record.
