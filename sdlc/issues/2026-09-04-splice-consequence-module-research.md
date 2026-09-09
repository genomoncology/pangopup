
# Splice-to-consequence module: research for the design

Prepared 2026-09-04, promoted into this repository on 2026-09-09. Every citation below was verified through BioMCP or Europe PMC on 2026-09-04.

This is research for a design, not an accepted plan. Pangopup owns the splice-score layer. A downstream variant engine owns transcript geometry and the clinical interpretation policy. The split of work described here is a proposal. That consumer settles it.

## The problem

Pangopup reports splice-site gain and loss with genomic offsets. Clinical interpretation needs the downstream consequence: which aberrant transcript forms, whether the reading frame shifts, whether a premature termination codon appears, whether nonsense-mediated decay degrades the transcript, and how strong the resulting loss-of-function evidence is under ACMG PVS1. No shipped tool owns that chain end to end.

## What already exists, verified

**Splice-score layer.** Pangolin, the model Pangopup wraps, is now published: "Generative modeling for RNA splicing prediction and design," eLife 2026, PMID 42213808. SpliceAI: Cell 2019, PMID 30661751. OpenSpliceAI, a modular retrainable reimplementation, eLife 2025, PMID 41165728. MutPred Splice, Genome Biology 2014, PMID 24451234, predicts exonic variants that disrupt splicing.

**Event-level models.** MMSplice, Genome Biology 2019, PMID 30823901, is the closest existing thing to a middle module. It models donor, acceptor, exon, and intron modules and predicts exon skipping and intron retention effects. It was the top performer in the CAGI 5 splicing challenge (Human Mutation 2019, PMID 31070280) on Vex-seq exon skipping and MaPSy splicing efficiency, and ships a VEP plugin that runs on VCF files.

**NMD layer.** aenmd, Bioinformatics 2023, PMID 37688563, an R package from kostkalab on GitHub, annotates PTC-containing transcripts for predicted NMD escape using established experimentally validated rules, at gnomAD and ClinVar scale. predNMD (BrennerLab, MIT license, pre-trained random forest) predicts NMD trigger and truncation type. TrunCat, PMID 42427729, remains closed. NMDetective, Nature Genetics 2019, PMID 31659324, supplies the feature set these tools draw on.

**Interpretation logic.** The PVS1 recommendations, PMID 30192042 (2018, 781 citations), define the ACMG decision tree for loss-of-function evidence, including the exon-skipping frameshift logic the module's output must feed.

**Cryptic-site biology.** Aberrant 5' splice sites in disease genes, PMID 17576681 (2007, 179 citations), is the classic analysis of cryptic site activation. Background splicing as a predictor of aberrant splicing, RNA Biology 2022, PMID 35188075, supports using background-junction rates as priors.

## The gap

The components exist. The assembly does not. MMSplice predicts event-level effects for the events its modules cover but does not assemble the aberrant transcript sequence or compute the frame and PTC position. aenmd takes stop-gain variants as input, assuming the consequence step already ran. VEP labels splice donor and acceptor variants without deciding the event. Nothing on GitHub or in the literature found today takes a splice-score output like Pangopup's and returns a PVS1-ready consequence call.

## Design

The middle module is mostly deterministic bioinformatics, not model training. The learning already lives in Pangopup. The design:

1. **Event hypothesis enumeration.** From Pangopup loss at a canonical site: exon skipping, intron retention, or activation of the strongest nearby cryptic site (candidates from the Pangopup gain scan plus a MaxEntScan-style rescore within the search window). From a deep intronic gain pair: pseudoexon inclusion between the gained donor and the nearest compatible acceptor. From an exonic or edge gain: alternative splice-site usage at the reported offset.
2. **Transcript assembly.** Apply each event to the transcript model (Ensembl GTF with MANE preference) and build the aberrant sequence. Pure sequence operations, deterministic, unit-testable.
3. **Frame and PTC analysis.** Exon lengths modulo three for skipping events, ORF scan of the assembled transcript, distance from new stop codon to the last exon-exon junction, the 50-to-55-nucleotide rule, long-exon and start-proximal adjustments following the validated NMD feature set.
4. **Score fusion.** Combine the Pangopup delta scores, a background-splicing prior for the event type, and the NMD call into a calibrated probability. This is the only place a small learned component is needed.
5. **Output.** Per-event consequence call, NMD trigger probability, PVS1-aligned evidence strength, and the predicted junction coordinates so a lab can design RNA validation assays against the exact predicted event.

## Datasets and benchmarks

1. **CAGI 5 splicing challenge data** (PMID 31070280): Vex-seq exon skipping and MaPSy splicing efficiency perturbation assays. Event-level ground truth, public, historical.
2. **100,000 Genomes Project splicing analysis** (Genome Medicine 2022, PMID 35883178): 38,688 genomes, near-splice and branchpoint constraint, RNA-confirmed new diagnoses. Clinical-scale ground truth. Access is controlled through a data application.
3. **Blood-based RNA-seq of 5,412 rare-disease individuals** (2026 preprint, no PMID yet at time of writing): observed aberrant junctions as ground truth for event calls.
4. **ClinVar splice-region pathogenic variants with RNA-confirmed consequences in submission notes**: the immediate regression set. aenmd's gnomAD and ClinVar annotation runs show this is mechanically available.
5. **gnomAD depletion**: predicted consequence-triggering variants should be rarer than tolerated ones. Unsupervised sanity check used by both SpliceAI and Pangolin validations.
6. **Ultra-deep RNA sequencing in Mendelian diagnostics** (AJHG 2025, PMID 41075783): high-sensitivity observed events for final validation.

Metrics: event-type accuracy against observed junctions, frameshift-call accuracy against RNA-confirmed consequences, NMD-call concordance between rules (aenmd), machine learning (predNMD), and allele-specific expression where available, PVS1 agreement with ClinVar pathogenic splice variants, and probability calibration.

## Effort shape and risks

Phase one, the deterministic core (assembly, frame, PTC, aenmd rules), is classical engineering testable against ClinVar immediately. Phase two, event priors and calibration, needs the CAGI and 100kGP benchmarks. Phase three, the predNMD integration and PVS1 output, is assembly work.

Hard parts: cryptic-site selection among many candidates (weak priors, the background-splicing literature helps), transcript isoform ambiguity (MANE-first policy), and deep-intronic pseudoexon pairing. Benchmark access for 100kGP is controlled; CAGI data are historical but public.

## Decisions this research opens

1. Build the deterministic core first and ship it without learned calibration, or hold release until calibrated on CAGI. Recommendation: deterministic core first; it is testable against ClinVar on day one and reversible.
2. NMD layer default: aenmd rules or predNMD machine learning. Recommendation: rules as the default (deterministic, auditable, clinically explainable), predNMD as an optional flag.
3. PVS1 output format: adopt the PMID 30192042 decision tree mapping directly, so the module's output drops into existing clinical interpretation workflows.
