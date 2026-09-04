---
flow: build
priority: 6
---
# The status route reports the enforced request contract

A client can ask whether the service is ready and which assets it opened, but it cannot discover the request limits and input vocabulary that the running service enforces. Consumers therefore copy PangoPup's maximum variants, maximum uncached model work, maximum allele length, assembly, and accepted contig forms into their own code. Those copies can drift after a PangoPup upgrade. A downstream annotation service already carries this duplicated policy.

The status route must report the active public scoring contract in machine-readable form. A client must be able to discover the API contract version, request-size limit, variant-count limit, uncached-model-work limit, allele-length limit, accepted assembly, and accepted contig forms without parsing prose. Every reported limit must agree with the value enforced by the scoring route in the same executable.

This changes a public contract consumed by service clients. A downstream annotation service can adopt the reported limits in place of local copies. Existing status fields remain available during that adoption.

Done, observably:

- One status response tells a client every enforced size, count, allele, assembly, and contig constraint needed to construct a scoring request.
- A mechanical test fails when an enforced limit changes without the reported contract changing with it.
- The status response exposes no filesystem path, credential, or host detail through the request contract.
- User documentation identifies the machine-readable contract as the source clients should use and retains human-readable limits for operators.

Boundary: do not add a combined scoring identity or change a scoring limit, accepted input, score, provenance object, asset format, cache key, or request behavior. Do not make readiness depend on a client reading the status route. Do not remove or rename an existing status field.
