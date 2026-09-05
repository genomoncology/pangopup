---
flow: build
priority: 7
---
# A caller can score an exact GRCh38 indel without supplying a padding base

PangoPup requires insertions and deletions in an anchored allele form. Some genomic systems hold an exact GRCh38 edit as an affected interval plus inserted or deleted bases. A downstream annotation service currently skips every such indel. Asking every client to manufacture the anchor duplicates reference access and creates an off-by-one risk at the scoring boundary.

PangoPup owns the exact installed GRCh38 reference used by the model. Both CLI `--variant` and HTTP `variants[]` accept two new strict uppercase string forms:

- `GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE` inserts `SEQUENCE` between adjacent one-based coordinates. `RIGHT` must equal `LEFT + 1`.
- `GRCh38:CONTIG:DEL:START:END:SEQUENCE` deletes `SEQUENCE` from the inclusive one-based interval `START..END`. The sequence length must equal `END - START + 1`.

The extra coordinate is deliberate. A shorter dash convention would make insertion side and deletion interval meaning implicit. Callers already holding an exact interval can provide the second coordinate without consulting a reference.

PangoPup always derives a left-anchored literal allele. For `INS:L:R:S`, it reads the base at `L` as anchor `A` and produces position `L`, REF `A`, ALT `A+S`. For `DEL:S:E:D`, it requires `S > 1`, reads `S-1..E`, verifies that `S..E` equals `D`, and produces position `S-1`, REF `A+D`, ALT `A`. It does not choose a right anchor when a left anchor is unavailable.

The inserted or deleted sequence contains 1 through 99 uppercase A/C/G/T bases. The derived longer allele therefore retains the existing 100-base limit. Empty sequences, delins, nonadjacent insertions, reversed or zero coordinates, arithmetic overflow, and sequence-length mismatch fail request validation without reference access. An out-of-contig interval, a deletion starting at the first base, or an installed anchor outside A/C/G/T also fails the complete request as `INVALID_REQUEST` because PangoPup cannot construct the literal allele fields required by an item outcome. Existing fixed model-context requirements still apply after conversion.

Grammar, coordinate syntax, interval geometry, alphabet, and length failures are request-validation failures that require no reference. Missing anchors, installed-contig boundary failures, and unsupported anchor symbols are request-level `INVALID_REQUEST` failures. Reference corruption and reference access failures remain request-level server failures.

A deletion whose submitted sequence disagrees with the installed reference becomes a typed item rejection when PangoPup read a valid A/C/G/T left anchor. PangoPup can construct the candidate literal tuple from that anchor and the submitted sequence, so the item uses the existing `MODEL_REJECTED` shape. Valid neighbors remain in an HTTP mixed-batch response. The public rejection stays generic and does not expose reference bytes. Conversion rejection runs no model inference and creates no cache entry. Adding a second rejected-item schema could preserve mixed results for failures with no literal tuple, but every client would then need another response union. This ticket accepts request-level failure for those cases instead.

Conversion occurs before routing, cache lookup, admission, and inference. Both the exact-edit and existing anchored forms converge on one `Grch38Variant`. Submitting equivalent forms produces the same score, provenance, response allele, queue weight, and persistent-cache identity. `pangopup-engine` owns the typed exact-edit values, conversion errors, and conversion against a supplied reference provider. The exact-edit form is a reference-dependent convenience at the model-routing boundary and becomes the existing canonical `Grch38Variant` before routing or caching. `pangopup-core`, the cache schema, and the cache key remain unchanged. CLI and HTTP parse and render.

This is a narrow convenience at the existing GRCh38 genomic-allele boundary. PangoPup continues to reject GRCh37, transcript HGVS, protein HGVS, symbolic structural variants, ambiguous coordinates, and requests for general normalization or alignment.

This changes a public input contract consumed by systems that already hold exact GRCh38 edits. After adoption, a downstream annotation service can send its exact insertion and deletion representation without querying a second GRCh38 reference merely to add a padding base.

Done, observably:

- Shared parser tests cover both exact-edit forms, every malformed field, strict operation and sequence spelling, coordinate geometry and overflow, sequence-length agreement, and the 99/100-base payload boundary.
- Engine tests use unequal neighboring bases and assert exact insertion and deletion conversion. They cover first and final reference boundaries, reversed deletion, nonadjacent insertion, deleted-sequence mismatch, unsupported anchor symbols, and provider failures.
- An inference spy proves that every conversion rejection stops before inference and cache insertion.
- CLI and HTTP accept both exact-edit forms. CLI covers ordinary lookup-first and `--model-only`. HTTP preserves a valid neighbor beside a deleted-sequence mismatch and returns request-level `INVALID_REQUEST` when no literal tuple can be constructed.
- Each exact edit and its equivalent anchored allele return byte-identical records, scores, warnings, provenance, and canonical response allele.
- Fresh persistent caches prove reuse in both directions: exact edit then anchored allele, and anchored allele then exact edit.
- Existing literal SNV, MNV, anchored insertion, and anchored deletion behavior remains covered.
- The full source-fingerprint tests prove that the checked SNV builder fingerprint, existing bundle manifests, and immutable SNV asset identity remain unchanged.
- `README.md`, CLI help, `spec/cli.md`, `spec/model-routing.md`, the HTTP contract and `spec/http-service.md` document the grammar and worked examples. `AGENTS.md` and `architecture/decisions/0003-standalone-genomic-variant-boundary.md` distinguish canonical core values from the engine-owned pre-canonical form. `architecture/runtime-data.md` records conversion before cache and inference.

Boundary: do not add GRCh37 liftover, general HGVS parsing, transcript projection, left or right alignment, equivalent-allele search, symbolic allele support, or a second reference source. Do not change the scoring model, precomputed SNV path, anchored input behavior, or literal-variant cache identity. Do not change `pangopup-core`, an artifact builder-source fingerprint, a checked bundle manifest, or an immutable asset identity for this input convenience.
