---
flow: build
priority: 9
deps: [0025]
---
# Strict JSON consumers adopt stable_gene before deployment

Ticket 0025 added `stable_gene` to every structured score record. `spec/http-service.md`, `spec/model-routing.md`, and `spec/snv-lookup.md` now show the field and explain its meaning. The public compatibility and release guidance does not state that the record shape changed. It also does not state an adoption order. A permissive JSON reader can ignore the new property. A strict reader that rejects unknown properties can reject the complete record.

The archived ticket also carries two unsupported premises. It names `GRCh38:chr7:140753336:A:T` as a production lookup and model example, but ticket 0025 did not verify that example against production assets. It says a downstream consumer had a comment that recorded silent drops, but no supporting comment existed in the reviewed evidence. Append-only history keeps the archived ticket and its record unchanged. The completion record for this remediation must correct both premises and must not cite either statement as evidence.

Publish one durable public compatibility contract for `stable_gene`. State that it adds a property to each structured JSON score record from the command-line and HTTP routes. State that permissive readers remain compatible and strict readers must accept the property before a PangoPup revision that emits it is deployed. Release-facing guidance for the first application release that contains the field must repeat that consumer-first order. A normal repository gate must protect the warning from accidental removal.

Keep the accepted ticket 0025 behavior. Removing `stable_gene` would restore the earlier record shape but would discard the stable grouping identity. Adding a new envelope version would expand the public interface again. This ticket chooses coordinated adoption and accepts the cost of deploying consumers before the producer.

Done, observably:

- Public structured-output documentation states that `stable_gene` changes the JSON score-record shape on both command-line and HTTP routes.
- The documentation distinguishes permissive readers from strict readers that reject unknown properties.
- Public release-facing guidance states that strict consumers must accept `stable_gene` before deployment of an application revision that emits it.
- A normal repository gate fails when the shape-change warning or consumer-first deployment order is absent.
- The guidance preserves the existing meanings of `gene` and `stable_gene`. It still requires consumers to retain `gene` when exact version or PAR identity matters.
- The completion record states that ticket 0025 did not verify its BRAF production example and had no supporting downstream comment. The archived ticket and record remain byte-identical.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change structured output, the human-readable table, scoring, routing, filtering, caching, assets, identities, versions, or release state. Do not publish, deploy, or require access to a downstream codebase. Do not edit any archived ticket, existing completion record, or historical release note. Do not name a private consumer.
