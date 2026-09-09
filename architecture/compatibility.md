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

- Every structured score record: adds `gene_names`.
- Every source-reference ambiguity: adds `gene_names`.
- Score-record `gene_names` object: carries `symbol`, `source`, `hgnc_id`, `ncbi_gene_id`, `prev_symbols`, and `alias_symbols`.
- Score-record `gene_names.source`: reports `hgnc` or `ncbi`.
- Status response root: adds `data_set_version`, `runtime_profile_id`, and `scoring_semantics`.

- Unnamed gene: the score record and the source-reference ambiguity carry no `gene_names` object, and a named gene omits `hgnc_id`, `ncbi_gene_id`, `prev_symbols`, and `alias_symbols` where its naming source supplies none.

Permissive JSON readers that ignore unknown properties remain compatible. Strict readers must accept the complete response-shape inventory before a producer emits it.

Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.5.0.

PangoPup ships one gene-name index with the build. No status field reports an installed naming vintage. A deployment cannot vary the names PangoPup returns.

`gene_names.source` names the authority that supplied the symbol. HGNC names a gene wherever HGNC reaches it. NCBI names the rest. A gene NCBI names carries no `hgnc_id`.

`prev_symbols` and `alias_symbols` are ambiguous. One symbol can point at several genes, and one can be another gene's approved symbol. Never match on them alone. `stable_gene` remains the only key.

The gene-name index does not move `scoring_identity`. A name changes no score. Unchanged scores stay one data-set version.

## Score values

A score value is one of 101 exact hundredths. `gain_score` runs from `0.00` through `1.00`. `loss_score` carries the same 101 magnitudes with its sign restored. It runs from `0.00` through `-1.00`. Every rendered score carries exactly two decimal places and one digit before the decimal point. A zero loss renders `0.00` and never `-0.00`.

The model rounds half to even. PangoPup multiplies the model value by 100, rounds half to even, and reports the resulting hundredth. A model value of 0.105 reports `0.10`. A model value of 0.115 reports `0.12`. Half-up would report `0.11` and `0.12`. The precomputed route rounds nothing. It reads exact hundredths from the published dataset.

A threshold finer than one hundredth cannot be evaluated. The representable neighbours of 0.106 are `0.10` and `0.11`. No PangoPup response distinguishes them. Pin a threshold on a hundredth and record which hundredth the deployment compared against.

A precomputed score and a modeled score are not interchangeable. The frozen upstream corpus carries both routes for four variants and five gene records, and one record disagrees on the value. `GRCh38:chr10:114306065:A:T` reports `0.06` at position 12 from the published dataset and `0.02` at position 13 from the model. No rate of disagreement is claimed. Positions diverge more widely than values. A zero score carries no meaningful position on either route. The published dataset reports position `-50` beside almost every zero score and reports some other position for roughly one zero score in 3,400. The model reports the position of its own extremum whatever that extremum rounds to. Read a position only where the score beside it is non-zero. Never infer a zero score from a position of `-50`, and never infer a position of `-50` from a zero score.

Store `provenance.kind` beside every score a system retains. That field names the route that produced the value. Without it a later comparison cannot tell one route's score from the other's.

A worker or thread setting changes no score. `--model-workers` and `--model-threads` move no score, no position, no status and no rejection reason. That statement rests on measurement. No gate proves it. The always-on gate runs a stand-in model with two operators. That model's arithmetic cannot reorder across threads. The production model was measured under thread counts 1, 4 and 8 and worker counts 1, 2 and 4, on one host and one build, against the shipped v0.5.0 assets. All six pairwise comparisons of those four runs were identical. Re-measure before citing the statement for another host, build or asset set. `--model-threads` does move the reported `effective_cpu_policy`. `scoring_identity` carries that policy and moves with it. `--model-workers` moves neither. The measured evidence is [`planning/artifacts/0040-score-value-determinism.md`](../planning/artifacts/0040-score-value-determinism.md), and [`spec/score-value.md`](../spec/score-value.md) states what each proof covers.

`scoring_identity` also hashes the PangoPup version. Read a moved identity only within one version. Two deployments of one PangoPup version on the same assets can differ in thread count. They report different identities and the same answers. A PangoPup version change moves the identity too, and a version change can move an answer.

## What a consumer pins

A consumer stores one value beside every retained score and compares it later. Store `data_set_version` as the data-set version when a system has one version field. `data_set_version` hashes the PangoPup version and the runtime profile identity. PangoPup hashes the RFC 8785 canonical `pangopup.scoring-data-set-version.v1` preimage over those two inputs. The status response publishes both of them. A consumer recomputes the value instead of trusting it.

`runtime_profile_id` hashes the whole admitted runtime profile. It covers every asset digest and every scoring input, `assembly`, `semantics`, `distance`, `masking_policy` and `cpu_policy` among them. No worker or thread setting moves `data_set_version` or `runtime_profile_id`.

`runtime_profile_id` alone is not the value to store. A PangoPup version change can move an answer with every asset unchanged. No such change moves `runtime_profile_id`. `data_set_version` covers the version too.

`scoring_identity` also hashes the effective CPU policy. A thread change moves it. Measurement on one host and one build found no score that a thread change moved. The `## Score values` section above states what that measurement covered and when to repeat it. `scoring_identity` keeps its value and its place on the status response and on every score item.

The runtime profile declares `cpu_policy` for the assets PangoPup qualified. The status `model.effective_cpu_policy` reports what the running service uses. A service started with `--model-threads 4` reports `sequential:4/1` as its effective policy while its runtime profile still declares `sequential:1/1`. Read the declared value as a property of the assets. Read the reported value as a property of the deployment.

The two scoring routes do not carry provenance to the same depth. A precomputed score item carries six provenance fields and a modeled score item carries twelve. A precomputed value ran no model, read no reference window and applied no runtime mask. `model_bundle_id`, `model_profile`, `effective_cpu_policy`, `reference_bundle_id`, `reference_profile`, `reference_sequence_set_sha256`, `mask_bytes` and `mask_sha256` describe none of it. A precomputed item still reports `masked` and `window`. Both values come from the published dataset manifest. `bundle_id`, `source_doi` and `source_archive_md5` pin the published dataset a precomputed value came from. A modeled score item carries none of those three fields. No modeled answer comes from the published dataset. Both routes answer under one scoring semantics. The status response reports it as `scoring_semantics`.
