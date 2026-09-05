---
flow: build
priority: 6
---
# A record reports one stable gene identity

The same gene arrives under two spellings depending on which route answered. A precomputed lookup of `GRCh38:chr7:140753336:A:T` reports `ENSG00000157764`. The model route reports `ENSG00000157764.14` for the same variant. Model results can also end in `_PAR_Y`.

Normal service use follows lookup-first routing. An SNV in the precomputed index takes the lookup path and an indel takes the model path, so one consumer receives both spellings for one gene and must strip the suffix before it can join anything. A caller can force the model through `model_only`, but doing that discards the authoritative lookup path and is not the normal integration policy. A downstream consumer already carries service-specific suffix handling. Its comment records the hazard: an unstripped versioned identifier stops matching and the record is silently dropped.

The gene filter already resolves all three accepted forms to one stable identity before filtering, so the service holds that identity and does not report it.

Report the stable gene component on every structured score record as `stable_gene`, beside the existing `gene`. Keep `gene` byte-identical to what the answering source reports. The new field carries the same stable Ensembl grouping and filter key that the gene filter already resolves, with no version suffix and no `_PAR_Y` suffix. Both routes then agree on that stable component. A downstream consumer can remove PangoPup-specific suffix handling when it groups or filters by the stable component, while retaining `gene` wherever exact version or PAR identity matters and retaining any normalization its own transcript data requires.

This is an additive field. A consumer that ignores it sees no change.

Done, observably:

- Every structured score record carries the stable gene component as `stable_gene`.
- One variant answered by the precomputed route and the same variant answered by the model route report the same `stable_gene` value.
- A record whose source gene ends in a version suffix or `_PAR_Y` reports `stable_gene` without either suffix.
- The reported `stable_gene` is the same value the gene filter resolves for that record's `gene`.
- The existing `gene` field keeps its exact current value on both routes.
- The HTTP specification and structured command-line output describe `stable_gene` as a grouping and filter key. They require consumers to retain `gene` wherever exact version or PAR identity matters.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change `gene`, scores, positions, statuses, provenance, HTTP statuses, response order, accepted gene filter forms, normalization, scoring, routing, caching, or limits. Do not add a gene symbol, a transcript, or any other annotation PangoPup does not already hold. Do not change the human-readable table, `pangopup-core`, or asset identities.
