# PangoPup v0.4.0 release notes

## Highlights

PangoPup v0.4.0 makes the HTTP batch service easier to consume. Every valid batch envelope without an operational failure returns HTTP 200 with one ordered outcome per submitted variant. This includes singleton batches and batches where every item is rejected. One rejected item no longer discards valid neighbors.

Every item echoes the exact submitted string. Rejected items carry a stable reason that distinguishes ordinary no-score outcomes from malformed values, reference mismatches, unavailable reference context, allele limits, unsupported shapes, and other model rejections. Structured records carry `stable_gene` beside the source-reported `gene` value.

`/v1/status` publishes the active scoring identity, the complete request contract, and model-work planning guidance. The model-work limit counts distinct canonical cache misses after the live cache lookup. Equivalent uncached model inputs within one request share one request-local admission unit and use at most one model execution. Temporary queue saturation returns `Retry-After`. A request above the live-cache model-work limit returns `MODEL_BATCH_TOO_LARGE` without `Retry-After`.

The CLI and service accept exact unpadded GRCh38 insertions and deletions. They also accept common mitochondrial contig spellings and stable, versioned, and `_PAR_Y` Ensembl gene identifiers. Asset failures now preserve the operating-system error.

The scoring assets remain unchanged. The scoring identity changes because the running software version is part of its input. Existing installed assets remain reusable through `pangopup sync --offline`.

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

The complete [`request_contract` nested schema](../../spec/http-service.md#request-contract-schema) is part of the public HTTP contract.

Permissive JSON readers that ignore unknown properties remain compatible. Strict readers must accept the complete response-shape inventory before a producer emits it.

Deploy strict consumer support for the complete response-shape inventory before deploying PangoPup v0.4.0.

`stable_gene` is the stable Ensembl grouping and filter key. `gene` remains the source-reported identity. Consumers must retain `gene` when exact version or PAR identity matters.

## Install

The immutable Linux x86-64 installer is:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.4.0/install.sh \
  | bash -s -- --version 0.4.0
```

The native Linux AMD64 and ARM64 container image is:

```bash
docker pull ghcr.io/genomoncology/pangopup:0.4.0
```

The executable and container remain thin. Run `pangopup sync` against the separately versioned immutable scoring assets before scoring.
