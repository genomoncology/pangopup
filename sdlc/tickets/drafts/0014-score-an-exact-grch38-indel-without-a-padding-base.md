---
flow: build
priority: 7
---
# A caller can score an exact GRCh38 indel without supplying a padding base

PangoPup requires insertions and deletions in an anchored allele form. Some genomic systems hold an exact GRCh38 edit as an affected interval plus inserted or deleted bases and use an empty-allele marker instead of copying a neighboring reference base into both alleles. A downstream annotation service uses that representation and currently skips every such indel. Asking every client to manufacture the anchor duplicates reference access and creates an off-by-one risk at the scoring boundary.

PangoPup owns the exact installed GRCh38 reference used by the model. It must accept a narrow input form that describes one unambiguous GRCh38 insertion or deletion without a caller-supplied padding base. The input must carry enough coordinate and sequence information to identify one exact edit. An ambiguous one-position dash convention is not sufficient.

PangoPup validates the edit against its active reference, derives the literal anchored allele used for routing and scoring, and reports that literal allele in the result. Submitting the equivalent accepted anchored allele produces the same score, provenance, and persistent-cache identity. A malformed interval, inconsistent deleted sequence, or edit outside the installed reference receives a typed client rejection before model inference.

This is a narrow convenience at the existing GRCh38 genomic-allele boundary. PangoPup continues to reject GRCh37, transcript HGVS, protein HGVS, symbolic structural variants, ambiguous coordinates, and requests for general normalization or alignment.

This changes a public input contract consumed by systems that already hold exact GRCh38 edits. After adoption, a downstream annotation service can send its exact insertion and deletion representation without querying a second GRCh38 reference merely to add a padding base.

Done, observably:

- A caller can submit one exact GRCh38 insertion and one exact GRCh38 deletion without supplying a shared padding base.
- Each unpadded edit and its equivalent accepted anchored allele return the same records, scores, warnings, and provenance.
- Both forms reuse the same persistent model result after either form has populated the cache.
- The response identifies the literal anchored GRCh38 allele that PangoPup actually scored.
- Ambiguous coordinates, inconsistent sequence, and reference-boundary errors receive stable client failures before model inference.
- The accepted coordinate meaning is documented with worked insertion and deletion examples.
- Tests cover reference boundaries and a context whose neighboring bases differ so an off-by-one implementation cannot pass on ordinary examples alone.

Boundary: do not add GRCh37 liftover, general HGVS parsing, transcript projection, left or right alignment, equivalent-allele search, symbolic allele support, or a second reference source. Do not change the scoring model, precomputed SNV path, anchored input behavior, or literal-variant cache identity.
