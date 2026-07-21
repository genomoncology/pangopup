# 0003 — Standalone genomic-variant boundary

Status: accepted
Date: 2026-07-21

## Decision

Pangopup is a standalone process and library with no dependency on Genome. Its
canonical input is a build-qualified genomic variant containing GRCh38 contig,
one-based position, reference allele, and alternate allele. A small fixed alias
table may recognize primary chromosome names and their exact RefSeq genomic
accessions.

Pangopup does not accept transcript/protein HGVS, project coordinates, infer a
variant from a gene phrase, or provide general gene annotation. An optional
Ensembl source-gene filter narrows lookup; without it Pangopup returns every
matching source record.

## Consequences

- Deployment is self-contained and needs no sibling service or database.
- Callers resolve transcript or protein descriptions before invoking Pangopup.
- Gene-specific masking remains truthful without making gene selection
  mandatory.
- Input errors remain distinguishable from absent precomputed annotations and
  unsupported model variants.
