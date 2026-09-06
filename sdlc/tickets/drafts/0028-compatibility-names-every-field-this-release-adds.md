---
flow: build
priority: 4
---
# Compatibility names every response-shape addition in 0.4

`architecture/compatibility.md` warns that a strict JSON reader must accept `stable_gene` before a producer emits it. The 0.4.0 candidate adds properties to several response objects that existed in 0.3, and the document names one.

The status root added `scoring_identity` and `request_contract`. The status model object added `work_unit`, `planning_millis_per_unit`, and `full_capacity_planning_seconds`. Every score item added `input` and `scoring_identity`. Rejected score items added `error` and `reason`. Every structured score record added `stable_gene`. A strict reader can reject any one of these additive properties. The new `request_contract` object also has a closed nested schema that a strict reader must adopt from the public HTTP contract.

A consumer that reads the compatibility document, updates its schema for `stable_gene`, and deploys still breaks on the other response additions. The document currently makes one record-level addition look like the only compatibility change.

Name every 0.4 property added to a response object that existed in 0.3, and say which response object carries each one. Link `request_contract` to the complete published nested schema rather than duplicating that schema in compatibility prose. Update both the durable compatibility contract and the 0.4.0 candidate release notes. Keep the existing guidance about retaining `gene` for exact version and PAR identity, because that guidance is correct and specific to `stable_gene`. State one consumer-first deployment order for the complete set.

Done, observably:

- The compatibility document names status-root `scoring_identity` and `request_contract`; status-model `work_unit`, `planning_millis_per_unit`, and `full_capacity_planning_seconds`; score-item `input` and `scoring_identity`; rejected-score-item `error` and `reason`; and structured-record `stable_gene`.
- `request_contract` links to the complete published nested schema that a strict reader must adopt.
- Each named property states which response object carries it.
- The compatibility document and 0.4.0 candidate release notes state one consumer-first deployment order for the complete set.
- The existing `stable_gene` guidance about `gene` and exact identity is unchanged.
- A normal repository gate fails if an inventory entry, its object scope, the request-contract schema link, or the consumer-first deployment order disappears from either required document. Mutation or removal tests prove the gate for each protected class.
- No behavior, field, value, status, or response shape changes.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: this is a documentation change. Do not change any response, field, value, status, code, message, limit, or executable behavior. Do not rewrite historical tickets, records, or publication evidence. Do not add a new property or a compatibility mechanism.
