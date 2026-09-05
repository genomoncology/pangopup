---
flow: build
priority: 4
---
# Compatibility names every field this release adds

`architecture/compatibility.md` warns that a strict JSON reader must accept `stable_gene` before a producer emits it. The 0.4.0 candidate adds four new properties, and the document names one.

Ticket 0022 added `input` to invalid items and ticket 0024 moved it to the front of every item. Ticket 0023 added `reason` beside `error` on every rejected item. Ticket 0026 added `planning_millis_per_unit` and `full_capacity_planning_seconds` to the status model object. Ticket 0025 added `stable_gene` to every structured record. Every one of them is an additive property that a strict reader rejects for the same reason, and `input` changes the first key of every score item.

A consumer that reads the compatibility document, updates its schema for `stable_gene`, and deploys still breaks on the other three. The document currently makes the least disruptive addition look like the only one.

Name every property this release adds to a published response, and say which response object carries each one. Keep the existing guidance about consumer-first deployment order and about retaining `gene` for exact version and PAR identity, because that guidance is correct and specific to `stable_gene`. State the deployment order once for the whole set rather than once per property.

Done, observably:

- The compatibility document names `input`, `reason`, `stable_gene`, `planning_millis_per_unit`, and `full_capacity_planning_seconds`.
- Each named property states which response object carries it: the score item, the structured score record, or the status model object.
- The consumer-first deployment order covers the complete set.
- The existing `stable_gene` guidance about `gene` and exact identity is unchanged.
- No behavior, field, value, status, or response shape changes.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: this is a documentation change. Do not change any response, field, value, status, code, message, limit, or executable behavior. Do not rewrite historical tickets, records, or publication evidence. Do not add a new property or a compatibility mechanism.
