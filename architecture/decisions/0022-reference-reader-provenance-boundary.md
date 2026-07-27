# 0022 — Reference reader and byte-producing provenance are separate

Status: accepted

## Decision

`PGRREF01` keeps one format and one implementation of each responsibility:
shared wire/layout and codecs, one writer, and one mmap reader/provider.
Ordinary, identified, qualification, and installed held-descriptor opens all
use that reader's structural parser and query decoder.

Installed admission may pass an already-authenticated member descriptor
through one explicit unsafe authority boundary. The returned capability is
opaque to safe callers, retains and maps that descriptor, and never reopens
`reference.pgr` or a parent directory by pathname.

Future reference builds use
`pangopup.reference-builder-source.v2`. Its inventory covers only the shared
wire/byte codecs, writer, byte-producing build adapter, causal error types,
root wiring, and locked dependencies. Reader/query code and post-build
certification/qualification code are not fingerprint inputs.

The mixed core source file is replaced in this fingerprint by
`reference-core-contract.v2`, a checked projection derived from all 25
compiled `Grch38Contig` code/display/from-code behaviors, including the
distinct `chrM` code. SNV v1 provenance remains unchanged.

## Consequences

- Reader-only, runtime-admission, or certification edits cannot churn future
  reference builder provenance.
- Byte-producing wire, writer, adapter, dependency, root-wiring, or projected
  contig behavior changes do change it.
- Existing v1 manifests remain readable. Builder provenance is descriptive,
  not a runtime compatibility key.
- Miniature v1 and v2 builds have identical `reference.pgr` and `NOTICE`
  bytes; only `builder.source_sha256` changes in their canonical manifests.
- The qualified production member and Ticket 024 profile are unchanged and
  were not opened or rebuilt for this migration.

This decision narrows the reference half of
[ADR 0012](0012-artifact-specific-builder-provenance.md); SNV provenance stays
as decided there.
