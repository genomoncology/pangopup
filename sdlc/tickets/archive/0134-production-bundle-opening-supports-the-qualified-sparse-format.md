---
flow: build
priority: 1
deps: []
---
# Production bundle opening supports the qualified sparse format

## Outcome

The production bundle opener accepts either the existing fixed-v1 format or the qualified sparse-direct-v1 format and exposes both through the unchanged score-provider behavior.

## What is true today

The complete sparse candidate passed the size, logical-parity, and warm gene-filtered latency gates. Its reader remains candidate-only. `BundleOpen` stores only `IndexReader`, and manifest admission rejects every format except `pangopup.fixed11.v1`, so no installed or explicit production route can open the qualified candidate.

## Scope

Add private format dispatch inside `pangopup-index`, keep fixed-v1 readable, and prove identical filtered and unfiltered lookup behavior with miniature bundles. Runtime lookup supports both formats. The existing fixed-only exhaustive-certification and benchmark accessors remain fixed-only and return a typed incompatibility for sparse input; a later asset-promotion ticket will extend certification deliberately. Update `architecture/index.md`, `architecture/runtime-data.md`, and `planning/frontier.md` to distinguish supported runtime opening from the still-active fixed-v1 release. Do not change the active release profile, publish an asset, change score precision, or change command-line or HTTP output.

## Done, observably

- A bundle manifest declaring `pangopup.sparse-direct.v1` opens through `BundleOpen` and answers filtered and unfiltered requests through the existing provider interface.
- Fixed-v1 opens and answers exactly as before.
- Unknown formats and mismatched manifest format, member media type, or payload format fail before lookup through the existing typed bundle error boundary. Canonical closed-manifest admission remains exact.
- Cross-format provider cases prove ordered overlaps, filtered overlaps, source-reference ambiguity, mixed records and ambiguity, misses, reference mismatches, unchanged source provenance, and the correctly derived format-specific bundle identity.
- Sparse touched-record corruption still fails at lookup, while bounded open does not scan an untouched ordinary payload.
- Fixed-v1 exhaustive certification and measured lookup remain unchanged. Calling either fixed-only operation on a sparse bundle returns a typed incompatibility rather than panicking or reporting fixed-v1 evidence.
- Focused index and asset tests, `make lint`, `make test`, and `make spec` pass.

The retained 0132 and 0133 size, logical-parity, and filtered-latency evidence remains applicable because this ticket does not alter either reader kernel. Unfiltered sparse performance remains unmeasured and the format remains inactive after this ticket.

## Dependencies

None. Tickets 0130 through 0133 supplied and qualified the candidate writer, reader, complete file, and latency evidence.
