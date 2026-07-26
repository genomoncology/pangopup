# ADR 0016: Lookup-first CLI model routing

Status: accepted

## Decision

`pangopup-engine` owns one small typed router above `VariantScorer`. It looks up
literal one-base substitutions first. Any filtered precomputed record or
source-reference ambiguity is authoritative. A pure SNV miss is model-required
when the caller enables fallback, and a non-SNV skips the SNV provider.

The CLI enables fallback only with the complete explicit local tuple
`--reference-bundle`, `--mask`, and `--model-bundle`. Without that tuple,
existing SNV lookup behavior—including a precomputed `not_found`—remains
byte-compatible; a non-SNV fails with `MODEL_ASSETS_REQUIRED`. This preserves
the established lookup-only interface while making the caller's decision to
invoke local model assets explicit.

The router emits an owned `ModelRequired` token. `ModelFallback` consumes that
token and binds one mutable scorer to the exact model identity, reference
provenance, descriptor-authenticated reference member, and descriptor-held
observed mask identity used by inference. It always scores and masks every
containing GENCODE gene before applying an optional stable Ensembl filter.

Modeled JSONL reports `pangopup-variant-score-v1`, the model/reference/mask
identities, masking/window settings, exact versioned GENCODE IDs, and ordered
warnings. Table output keeps the existing compact columns. Complete batches are
buffered before stdout is written.

## Why

Published SNV scores are faster and authoritative. Model inference is needed
only for caller-enabled lookup misses and supported non-SNVs. Collecting route
decisions before opening fallback assets keeps the hit path unchanged and
allows one reference, mask, and model open for a mixed batch.

An observed mask hash alone could be paired accidentally with different mask
bytes. `open_identified` therefore returns one coupled capability whose queries
and identity share the same retained regular, single-link descriptor and mmap.
It identifies caller-supplied bytes; it does not declare an arbitrary tuple
compatible or trusted.

The explicit reference path uses the corresponding stronger open: hash the
complete bounded member through one held descriptor, verify the
manifest-declared digest, then map and retain that descriptor in the only
reference capability accepted by `ModelFallback`. This prevents valid
manifest provenance from being paired with same-size substituted sequence
bytes. Ordinary installed-runtime reference open remains cheap; the explicit
path pays the complete-member read only when fallback is actually required.

## Boundaries

This supersedes ADR 0015 only where that decision kept routing and gene
filtering outside `pangopup-engine`. `VariantScorer` itself remains unfiltered
and owns the same fixed scoring algorithm.

This decision does not add automatic asset installation, a coherent four-asset
profile, model caching, CPU tuning/batching, HTTP, Docker, systemd, HGVS, or
normalization.
