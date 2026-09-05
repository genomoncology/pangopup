---
flow: build
priority: 4
---
# Every caller-visible service fact has one source

Three caller-visible facts are each assembled in two places. Every copy agrees today, but a later edit can make status, provenance, accepted input, or refusal text disagree with actual behavior.

The service computes its effective CPU policy for scoring provenance, cache identity, and active scoring identity, then reconstructs the same policy separately for status. The scoring adapters and status contract share their mitochondrial spellings, but the maintenance reference command repeats the `MT` and `chrMT` aliases. Request enforcement and status share the 100-item and 10-uncached-item limits, but their refusal messages repeat those numbers as text.

Use one runtime value for the effective CPU policy everywhere the service reports or hashes it. Use one caller-facing contig policy across the scoring adapters, status contract, and maintenance reference command. Build request-limit refusals from the same values used by enforcement and status.

Keep source ingestion stricter than caller input. It must continue to reject mitochondrial aliases where canonical stored identities are required. Keep `pangopup-core` unchanged because its source participates in immutable asset fingerprints. Existing shared reference-accession behavior provides the common boundary for caller contig spellings.

Done, observably:

- Status, result provenance, cache identity, and active scoring identity use the effective CPU policy that the service computed once.
- Scoring inputs, the reported contract, and the maintenance reference command accept the same caller-facing mitochondrial spellings and produce `chrM`.
- Source ingestion still rejects `MT` and `chrMT`.
- Each request-limit refusal states the value that the same executable reports through `request_contract`.
- Behavioral tests cover the status and scoring policy, every mitochondrial adapter, strict ingestion, and both limit refusals without relying on source-text inspection.
- Existing values, messages, codes, statuses, response shapes, scoring, routing, admission, and cache behavior remain unchanged.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change a limit, alias, CPU policy, error code, HTTP status, response shape, accepted gene or variant form, or reported score. Do not change `pangopup-core`, builder source fingerprints, manifests, asset identities, source-ingestion vocabulary, admission behavior, scoring, routing, or caching. Do not add configuration, status fields, execution modes, or thread controls.
