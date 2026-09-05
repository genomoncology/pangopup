---
flow: build
priority: 7
---
# A rejected item says why it was rejected

Ticket 0022 gave every submitted variant its own outcome so a batch client can react to each one. It separates invalid values under `INVALID_VARIANT` from normalized model refusals under `MODEL_REJECTED`, but each class still collapses materially different causes into one code and message.

`ModelRejection` distinguishes six reasons: an unsupported variant shape, alleles above the model limit, insufficient reference context, a submitted reference allele that disagrees with GRCh38, a reference window symbol the model cannot score, and a position no GENCODE gene covers. The scoring route collapses all six into `MODEL_REJECTED` with the message `scoring failed`. `INVALID_VARIANT` collapses malformed literals, unsupported genomic values, and exact-edit geometry failures the same way.

Five materially different situations receive the same rejection classification today. A reference-allele disagreement, an ordinary intergenic position, a position past the end of a contig, an allele above the reported model limit, and a deletion sequence that disagrees with the reference are indistinguishable through their error fields.

An intergenic position is an ordinary no-score outcome. Other reasons can identify caller data, a supported-model boundary, or unavailable reference context. A downstream client must currently treat every rejection as neutral, so a systematic coordinate, liftover, or normalization defect remains indistinguishable from ordinary intergenic input. A caller that exceeds the reported `max_model_allele_bases` receives an opaque item, while every request-level limit violation names its number.

Give each rejected item a stable machine-readable reason drawn from a closed published vocabulary. Keep `status`, `error.code`, and `error.message` exactly as they are so a consumer that ignores the reason is unaffected. Separate at least these outcomes: the position lies outside every annotated gene, the submitted reference allele disagrees with GRCh38, the reference window is unavailable at that position, the alleles exceed the reported model limit, the variant shape is unsupported, and the reference window holds a symbol the model cannot score. Cover invalid item values with the same mechanism so a client can separate a malformed literal from a rejected exact-edit geometry.

A reason is a stable slug, not backend text. It must not carry window offsets, byte values, sequences, coordinates, paths, or any other internal detail. Publish the complete vocabulary in the current HTTP contract documentation. A client must handle an unknown future reason safely rather than treating the vocabulary as permanently exhaustive. The command-line interface already prints the specific human-readable message and keeps that behavior unchanged.

Done, observably:

- Every rejected item carries a reason from the published closed vocabulary.
- A position outside every annotated gene is distinguishable from a reference-allele disagreement through the response alone.
- An allele above the reported model limit, a position past the end of a contig, an unsupported variant shape, and an unscorable reference symbol each report the applicable published reason.
- An invalid item value reports whether the literal was malformed, the genomic values were unsupported, or the exact-edit geometry failed.
- `error.code` and `error.message` values are byte-identical to their current values for every case.
- No reason value contains an offset, byte value, sequence, coordinate, or path.
- A client can read the complete reason vocabulary in the public HTTP contract without reading source.
- Command-line rejection messages are unchanged.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change HTTP statuses, response order, item shape beyond the added reason, accepted variant or gene forms, normalization, scoring, routing, caching, admission accounting, limits, or the split between item outcomes and request-level failures. Do not expose backend error text, internal offsets, or provider detail. Do not change `pangopup-core` rejection semantics or the command-line interface. Do not add configuration or a new endpoint.
