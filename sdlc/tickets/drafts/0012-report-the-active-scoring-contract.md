---
flow: build
priority: 6
deps: ["0014"]
---
# The status route reports the enforced request contract

A client can ask whether the service is ready and which assets it opened, but it cannot discover the request limits and input vocabulary that the running service enforces. Consumers therefore copy PangoPup's maximum variants, maximum uncached model work, maximum allele length, assembly, and accepted contig forms into their own code. Those copies can drift after a PangoPup upgrade. A downstream annotation service already carries this duplicated policy.

The status route must report the active public scoring contract in machine-readable form. A client must be able to discover the API contract version, request-size limit, variant-count limit, uncached-model-work limit, allele-length limit, accepted assembly, and accepted contig forms without parsing prose. Every reported limit must agree with the value enforced by the scoring route in the same executable.

The change adds this exact `request_contract` object to `/v1/status` and preserves every existing field:

```json
{
  "request_contract": {
    "api_version": "v1",
    "route": "/v1/score",
    "content_type": "application/json",
    "max_body_bytes": 65536,
    "variants": {
      "min_items": 1,
      "max_items": 100,
      "max_uncached_model_items": 10,
      "model_work_unit": "uncached_model_variant",
      "assembly": "GRCh38",
      "max_model_allele_bases": 100,
      "max_exact_edit_sequence_bases": 99,
      "forms": [
        "GRCh38:CONTIG:POS:REF:ALT",
        "GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE",
        "GRCh38:CONTIG:DEL:START:END:SEQUENCE"
      ],
      "contigs": [
        {
          "canonical": "chr1",
          "accepted": ["1", "chr1", "NC_000001.11"]
        }
      ]
    },
    "gene_filter": {
      "accepted_forms": [
        "ENSG###########",
        "ENSG###########.VERSION",
        "ENSG###########.VERSION_PAR_Y"
      ],
      "version_minimum": 1,
      "version_maximum": 4294967295,
      "version_allows_leading_zero": false
    },
    "model_only": {
      "type": "boolean",
      "optional": true
    }
  }
}
```

The `contigs` array contains all 25 primary contigs. Autosomes, X, and Y each report their bare, `chr`, and exact RefSeq accession forms. The mitochondrial entry reports `M`, `MT`, `chrM`, `chrMT`, and `NC_012920.1`, with `chrM` as canonical output. The example above shows one entry rather than the complete array.

`api_version` identifies the `/v1/score` request contract. It does not replace the software `version`, active `scoring_identity`, or route provenance. A breaking scoring-request change requires a new API version. Adding an unrelated status field does not.

One service request-contract definition owns the body, item, and uncached-work limits. Status serialization and scoring enforcement consume those same values. The engine exports its model allele and exact-edit sequence limits through named constants or accessors. Engine validation and status reporting consume those same values. The adapter generates reported contig descriptors from the same helper used by `parse_contig`, and exact RefSeq accessions continue to come from `required_accession`. Do not change `pangopup-core`; its source participates in immutable asset fingerprint evidence.

The model allele limit describes model eligibility. Literal parsing still accepts any nonempty uppercase A/C/G/T allele. A literal allele over 100 bases reaches the existing model-request rejection when its route requires the model. The exact-edit limit is 99 bases because conversion adds one anchor base. The status contract must not claim that HTTP parsing rejects every literal allele over 100 bases.

The contract includes all three current variant forms and the stable, versioned, and versioned `_PAR_Y` gene-filter forms. This ticket does not change parsing or reported gene identities.

This changes a public contract consumed by service clients. A downstream annotation service can adopt the reported limits in place of local copies. Existing status fields remain available during that adoption.

Done, observably:

- One status response tells a client every enforced size, count, allele, assembly, and contig constraint needed to construct a scoring request.
- The full additive status shape is pinned, and all existing status fields remain unchanged.
- Tests read each numeric boundary from serialized status, then exercise the enforced boundary and one value over it for body bytes, variant count, uncached model work, model allele length, and exact-edit sequence length.
- Tests iterate every reported contig spelling through the actual parser and verify its reported canonical value. Coverage includes all RefSeq accessions and mitochondrial aliases.
- Tests exercise one accepted value for each reported variant form and gene-filter form, including the documented version-number boundaries.
- The contract stays identical across ready, draining, and failed service state and across queue occupancy changes.
- The object contains only fixed public vocabulary and numeric limits. It contains no path, listener address, credential, request content, hostname, or other host detail.
- Existing status and result scoring-identity tests remain unchanged in meaning. The request contract does not enter or change `scoring_identity`.
- User documentation identifies the machine-readable contract as the source clients should use and retains human-readable limits for operators.

Boundary: do not add a combined scoring identity or change a scoring limit, accepted input, score, provenance object, asset format, cache key, or request behavior. Do not make readiness depend on a client reading the status route. Do not remove or rename an existing status field.
