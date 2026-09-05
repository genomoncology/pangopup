---
flow: build
priority: 8
---
# Every item carries the string that produced it

Ticket 0022 gave invalid items an `input` field holding the exact submitted string. Every other item omits it and reports normalized genomic fields instead.

A batch client therefore cannot correlate a result with its request by content. `chr7`, `7`, and `NC_000007.14` all come back as `chr7`, and an exact edit comes back as an anchored literal, so the submitted string never appears in a normal result. The only correlation left is position in the array. A downstream consumer already carries about thirty lines that pair results positionally, checksum the returned position against the submitted one, and drop a whole chunk on any disagreement. That client will deduplicate submitted strings and retain one input-to-callers mapping before each request. The response should let it use that mapping directly.

Report the exact submitted string as `input` on every item, unchanged and in the order received. Do not alter, canonicalize, trim, or re-encode it. Every other field keeps its current name, value, and meaning. Correlation then works the same way for a found item, a normalized rejection, and an invalid value. A client that submits unique strings can map results by content without trusting array position. Duplicate occurrences remain ordered occurrences of the same input and do not become distinct identifiers.

This is an additive field. A consumer that ignores `input` sees no change.

Done, observably:

- Every item in a `/v1/score` response carries `input` holding the exact submitted string.
- A contig alias, an accession, and an exact-edit form each return the string the caller sent, not the canonical form.
- A found item, a `MODEL_REJECTED` item, and an `INVALID_VARIANT` item all carry `input` in the same place with the same meaning.
- Duplicate submitted strings each return their own ordered item carrying that string; `input` does not claim to distinguish duplicate occurrences.
- Every existing field keeps its current name, value, meaning, and relative order.
- Count, membership, duplicate, and response-shape validation remain necessary even when a client stops using positional correlation.
- The HTTP specification describes `input` as the submitted-value correlation field.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change HTTP statuses, response order, item statuses, error codes, error messages, normalization, scoring, routing, caching, admission accounting, or limits. Do not add a caller-supplied identifier, a request echo object, or a new endpoint. Do not remove or rename an existing item field.
