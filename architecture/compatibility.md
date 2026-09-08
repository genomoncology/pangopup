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

## Score values

A score value is one of 101 exact hundredths. `gain_score` runs from `0.00` through `1.00`. `loss_score` carries the same 101 magnitudes with its sign restored. It runs from `0.00` through `-1.00`. Every rendered score carries exactly two decimal places and one digit before the decimal point. A zero loss renders `0.00` and never `-0.00`.

The model rounds half to even. PangoPup multiplies the model value by 100, rounds half to even, and reports the resulting hundredth. A model value of 0.105 reports `0.10`. A model value of 0.115 reports `0.12`. Half-up would report `0.11` and `0.12`. The precomputed route rounds nothing. It reads exact hundredths from the published dataset.

A threshold finer than one hundredth cannot be evaluated. The representable neighbours of 0.106 are `0.10` and `0.11`. No PangoPup response distinguishes them. Pin a threshold on a hundredth and record which hundredth the deployment compared against.

A precomputed score and a modeled score are not interchangeable. The frozen upstream corpus carries both routes for four variants and five gene records, and one record disagrees on the value. `GRCh38:chr10:114306065:A:T` reports `0.06` at position 12 from the published dataset and `0.02` at position 13 from the model. No rate of disagreement is claimed. Positions diverge more widely than values. The published dataset reports position `-50` wherever its score is zero. The model reports the position of its own extremum whatever that extremum rounds to. Read a position only where the score beside it is non-zero.

Store `provenance.kind` beside every score a system retains. That field names the route that produced the value. Without it a later comparison cannot tell one route's score from the other's.

A worker or thread setting changes no score. `--model-workers` and `--model-threads` move no score, no position, no status and no rejection reason. `--model-threads` does move the reported `effective_cpu_policy`. `scoring_identity` carries that policy and moves with it. `--model-workers` moves neither. The measured evidence is [`planning/artifacts/0040-score-value-determinism.md`](../planning/artifacts/0040-score-value-determinism.md), and [`spec/score-value.md`](../spec/score-value.md) states what each proof covers.

`scoring_identity` also hashes the PangoPup version. Read a moved identity only within one version. Two deployments of one PangoPup version on the same assets can differ in thread count. They report different identities and the same answers. A PangoPup version change moves the identity too, and a version change can move an answer.
