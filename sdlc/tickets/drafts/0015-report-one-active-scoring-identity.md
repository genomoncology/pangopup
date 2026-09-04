---
flow: build
priority: 6
---
# Every score reports one active scoring identity

PangoPup reports detailed route provenance and lists its active asset identities through the status route. A downstream annotation system has room for one data-set version. It therefore asks an operator to configure a second version string by hand. That string can disagree with the image, installed assets, scoring semantics, or CPU policy that produced the score.

PangoPup must report one concise active scoring identity through the status route and every scoring result. The identity changes whenever an installed input or active policy capable of changing an answer changes. Detailed precomputed and modeled provenance remains available and authoritative. The concise identity names the complete active scoring environment so a consumer with one version field can record the truth without reconstructing it.

The identity must remain stable across restarts and machines that use identical scoring inputs and policy. Mutable cache contents, queue state, process identity, filesystem paths, and host details must not affect it.

This changes a public contract consumed by scoring clients. A downstream annotation service can remove its manually configured PangoPup data-set version after it adopts a release carrying this identity.

Done, observably:

- The status response and every `found` or `not_found` result report the same active scoring identity.
- Two services with identical scoring inputs and policy report the same identity.
- Changing any scoring input or policy capable of changing an answer changes the identity.
- Restarting the same service with unchanged inputs and policy does not change the identity.
- Cache contents, queue state, worker process identity, filesystem paths, and host details do not affect or appear in the identity.
- Detailed route provenance remains unchanged and lets a caller audit the components represented by the concise identity.
- User documentation tells consumers when to store the concise identity and when to retain the detailed route provenance.

Boundary: do not change score calculation, provenance values, asset formats, cache identity, request limits, input parsing, routing, or readiness. Do not remove or rename an existing status or result field.
