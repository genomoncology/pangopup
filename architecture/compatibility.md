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

## v0.5.0 response-shape inventory

PangoPup v0.5.0 changes response shapes from v0.4.1:

- Status response root: adds `naming`.
- Status `naming` object: carries `available` and `release`.
- Every structured score record: adds `gene_names`.
- Every source-reference ambiguity: adds `gene_names`.
- Score-record `gene_names` object: carries `symbol`, `hgnc_id`, `ncbi_gene_id`, `prev_symbols`, and `alias_symbols`.

- Unnamed gene: the score record and the source-reference ambiguity carry no `gene_names` object, and a named gene omits `ncbi_gene_id`, `prev_symbols`, and `alias_symbols` where the naming source supplies none.

Permissive JSON readers that ignore unknown properties remain compatible. Strict readers must accept the complete response-shape inventory before a producer emits it.

Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.5.0.

`naming.available` is `false` when no naming source is installed. Every score record and every source-reference ambiguity then reports its Ensembl accession alone. `naming.release` names the installed naming vintage. A consumer reads it to tell one vintage from another.

`prev_symbols` and `alias_symbols` are ambiguous. One symbol can point at several genes, and one can be another gene's approved symbol. Never match on them alone. `stable_gene` remains the only key.

Installing, updating or removing a naming source does not move `scoring_identity`. A naming refresh changes no score. Unchanged scores stay one data-set version.
