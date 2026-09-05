---
flow: build
priority: 6
---
# A record reports one stable gene identity

The same gene arrives under two spellings depending on which route answered. A precomputed lookup of `GRCh38:chr7:140753336:A:T` reports `ENSG00000157764`. The model route reports `ENSG00000157764.14` for the same variant. Model results can also end in `_PAR_Y`.

A caller does not choose its route. An SNV in the precomputed index takes the lookup path and an indel takes the model path, so one consumer receives both spellings for one gene and must strip the suffix before it can join anything. A downstream consumer already carries that parser. Its comment records the hazard: an unstripped versioned identifier stops matching and the record is silently dropped.

The gene filter already resolves all three accepted forms to one stable identity before filtering, so the service holds that identity and does not report it.

Report the stable gene identity on every record as its own field, beside the existing `gene`. Keep `gene` byte-identical to what the answering source reports, because it names the exact annotation release that produced the score. The new field carries the stable Ensembl identifier that the gene filter already resolves, with no version suffix and no `_PAR_Y` suffix. Both routes then agree on one join key for one gene.

This is an additive field. A consumer that ignores it sees no change.

Done, observably:

- Every record carries the stable gene identity as its own field.
- One variant answered by the precomputed route and the same variant answered by the model route report the same stable identity.
- A record whose source gene ends in a version suffix or `_PAR_Y` reports the stable identity without either suffix.
- The reported stable identity is the same value the gene filter resolves for that record's `gene`.
- The existing `gene` field keeps its exact current value on both routes.
- The HTTP specification and the command-line output describe both fields and which one to join on.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change `gene`, scores, positions, statuses, provenance, HTTP statuses, response order, accepted gene filter forms, normalization, scoring, routing, caching, or limits. Do not add a gene symbol, a transcript, or any other annotation PangoPup does not already hold. Do not change `pangopup-core` or asset identities.
