# 0002 — CLI first observable surface

Status: accepted
Date: 2026-07-20

## Decision

The initial behavior is exercised through a command-line program backed by the
same Rust library future transports will use. The first complete slice is exact
GRCh38 SNV lookup from the precomputed index. REST, model inference, and a
non-SNV result cache are deferred.

## Consequences

- Format correctness and performance can be measured without network or JSON
  server overhead.
- `spec/*.md` can define the outside-in contract with real commands.
- A future REST service is an adapter, not a second implementation.
- The CLI/process boundary remains independently useful for scripts, testing,
  benchmarking, and deployments that do not need HTTP.
