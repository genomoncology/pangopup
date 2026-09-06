# Compatibility

## v0.4.0 response-shape inventory

PangoPup v0.4.0 changes response shapes from v0.3.0:

- Status response root: adds `scoring_identity` and `request_contract`.
- Status `model` object: adds `work_unit`, `planning_millis_per_unit`, and `full_capacity_planning_seconds`.
- Every score item: adds `input` and `scoring_identity`.
- Score item status: adds `"rejected"`.
- Rejected score item: carries `error` and `reason`.
- Every structured score record: adds `stable_gene`.

- Invalid-input rejected item: carries `input`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no normalized genomic fields or `provenance`.
- Normalized model-rejected item: carries `input`, normalized `assembly`, `contig`, `position`, `ref`, and `alt`, `status`, empty `records`, empty `source_reference_ambiguities`, `error`, `reason`, and `scoring_identity`; it has no `provenance`.

The complete [`request_contract` nested schema](../spec/http-service.md#request-contract-schema) is part of the public HTTP contract.

Permissive JSON readers that ignore unknown properties remain compatible. Strict readers must accept the complete response-shape inventory before a producer emits it.

Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.4.0.

PangoPup published no v0.4.0 container. The first published container carrying this inventory is v0.4.1.

`stable_gene` is the stable Ensembl grouping and filter key. `gene` remains the source-reported identity. Consumers must retain `gene` when exact version or PAR identity matters.
