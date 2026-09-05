---
flow: build
priority: 6
---
# A rejected item says why it was rejected

Ticket 0022 gave every submitted variant its own outcome so a batch client can react to each one. A client still cannot react differently, because every declined item carries the same `error.code` and the same message.

`ModelRejection` distinguishes six reasons: an unsupported variant shape, alleles above the model limit, insufficient reference context, a submitted reference allele that disagrees with GRCh38, a reference window symbol the model cannot score, and a position no GENCODE gene covers. The scoring route collapses all six into `MODEL_REJECTED` with the message `scoring failed`. `INVALID_VARIANT` collapses malformed literals, unsupported genomic values, and exact-edit geometry failures the same way.

Five materially different situations return identical items today. A reference-allele disagreement, an ordinary intergenic position, a position past the end of a contig, an allele above the reported model limit, and a deletion sequence that disagrees with the reference are indistinguishable over HTTP.

Only one of those five means nothing is wrong. The rest mean the caller has a defect to fix. A downstream client is therefore told to treat every rejection as a neutral no-score outcome, so a systematic coordinate, liftover, or normalization defect stays invisible for as long as it exists. A caller that exceeds the reported `max_model_allele_bases` receives an opaque item, while every request-level limit violation names its number.

Give each rejected item a stable machine-readable reason drawn from a closed published vocabulary. Keep `status`, `error.code`, and `error.message` exactly as they are so a consumer that ignores the reason is unaffected. Separate at least these outcomes: the position lies outside every annotated gene, the submitted reference allele disagrees with GRCh38, the reference window is unavailable at that position, the alleles exceed the reported model limit, the variant shape is unsupported, and the reference window holds a symbol the model cannot score. Cover invalid item values with the same mechanism so a client can separate a malformed literal from a rejected exact-edit geometry.

A reason is a stable slug, not backend text. It must not carry window offsets, byte values, sequences, coordinates, paths, or any other internal detail. Publish the complete vocabulary where a client discovers the rest of the contract, so a client can enumerate every value it must handle. The command-line interface already prints the specific human-readable message and keeps that behavior unchanged.

Done, observably:

- Every rejected item carries a reason from the published closed vocabulary.
- A position outside every annotated gene is distinguishable from a reference-allele disagreement through the response alone.
- An allele above the reported model limit, a position past the end of a contig, an unsupported variant shape, and an unscorable reference symbol each report their own reason.
- An invalid item value reports whether the literal was malformed, the genomic values were unsupported, or the exact-edit geometry failed.
- `error.code` and `error.message` values are byte-identical to their current values for every case.
- No reason value contains an offset, byte value, sequence, coordinate, or path.
- A client can read the complete reason vocabulary from the service without reading source.
- Command-line rejection messages are unchanged.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change HTTP statuses, response order, item shape beyond the added reason, accepted variant or gene forms, normalization, scoring, routing, caching, admission accounting, limits, or the split between item outcomes and request-level failures. Do not expose backend error text, internal offsets, or provider detail. Do not change `pangopup-core` rejection semantics or the command-line interface. Do not add configuration or a new endpoint.
