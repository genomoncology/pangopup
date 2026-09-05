---
flow: build
priority: 8
deps: ["0004"]
---
# A rejected model variant does not discard valid results in the same batch

The HTTP scoring route fails a complete batch when the model rejects one variant. A wrong GRCh38 reference base is enough to discard scores for every valid variant beside it. A downstream annotation service compensates by repeatedly dividing a failed batch and sending each half again. That workaround turns one request into many requests and also multiplies genuine server failures.

Ticket 0004 gives a model rejection the correct client status when the complete request is rejected. This ticket covers the mixed-batch case. A syntactically valid request that contains at least one answerable variant returns the normal success envelope and one ordered outcome for every input. A model rejection appears as a typed outcome for its own variant. It does not discard `found` or `not_found` results for other variants.

The mixed response uses HTTP 200. A rejected outcome keeps the submitted variant's normalized `assembly`, `contig`, `position`, `ref`, and `alt` fields. It carries `status: "rejected"`, empty `records` and `source_reference_ambiguities` arrays, and `error: {"code":"MODEL_REJECTED","message":"scoring failed"}`. It omits provenance because no score source produced an answer. Normal outcomes keep their exact current shape.

The contract uses 200 because the request produced usable results. A request-wide 422 would force clients to recover valid results through an error path. HTTP 207 would introduce an uncommon envelope status without removing item inspection. The accepted cost of 200 is that callers must inspect each outcome status before treating the batch as wholly successful. The all-rejected case retains 422 so a caller can still detect a wholly unusable request from the status line.

The existing distinction between a rejected variant and an operational failure remains authoritative. A model refusal caused by the submitted allele belongs to that item. A scoring failure, unusable cache, unavailable worker, or failed service remains a request-level failure and must not be presented as an item rejection or a normal absence.

Existing callers that submit only answerable variants receive the same result shape and values they receive today. A request where every model variant is rejected retains ticket 0004's 422 behavior. Malformed JSON and request-wide validation failures retain their current request-level answers.

This changes a public contract consumed by downstream batch clients. After a release carrying this behavior is deployed, a downstream annotation service can remove recursive HTTP-error bisection and consume the ordered item outcomes directly.

Done, observably:

- A batch with one scoreable variant and one model-rejected variant returns the scoreable variant's normal answer exactly once.
- The same HTTP 200 response carries the defined stable typed rejection for the rejected input in its original position.
- A `not_found` result beside a rejected item remains a normal `not_found` result.
- A genuine scoring, cache, worker, or readiness failure still fails the request and never masquerades as an item rejection or `not_found`.
- A batch containing only model-rejected variants receives the client status established by ticket 0004.
- The HTTP documentation explains which failures belong to one item and which failures invalidate the request.
- The suite proves the mixed-batch behavior and preserves the exact successful results for the unaffected variants.

Boundary: do not add GRCh37 input, allele normalization, retry behavior, asynchronous jobs, or partial results after an operational failure. Do not expose the backend's detailed rejection message. Do not change the CLI, score calculation, lookup-first routing, result order, caching identity, or the behavior of a batch whose variants all produce normal results.
