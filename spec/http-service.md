# Foreground HTTP service

The executable exposes one foreground service command with explicit bounded
model capacity. Invalid capacity fails before opening assets or binding a
listener.

```bash
pangopup serve --help | mustmatch like 'Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]

Run the foreground HTTP scoring service. --model-queue-capacity counts running and queued uncached model variants and defaults to 20. With one worker, that default gives a planning estimate of about 205 seconds from the slowest retained p50. The estimate is not a latency guarantee.'
```

```bash run id=serve-invalid-workers exit=2 stream=stderr
pangopup serve --model-workers 0
```

```text expect=serve-invalid-workers exact
{"status":"error","code":"CLI_USAGE","message":"--model-workers must be in 1..=8","details":null}
```

The service never downloads during startup. A missing installed profile fails
before bind with the existing path-free asset error and directs the operator to
run `pangopup sync`.

```bash run id=serve-missing-assets exit=1 stream=stderr
rm -rf ../target/spec/missing-service-data
pangopup serve --data-dir "$(pwd)/../target/spec/missing-service-data"
```

```text expect=serve-missing-assets exact
{"status":"error","code":"ASSETS_MISSING","message":"required assets are missing; run pangopup sync","details":null}
```

The inside-out HTTP tests inject miniature providers and exercise the actual router without downloading or running the production model. They pin exact success/error bytes, lookup and SQLite bypass under saturation, whole-request FIFO admission by uncached model variant, exact-boundary 429 backpressure with retry guidance, HTTP 422 rejection above the live-cache-dependent request limit, disconnect accounting, worker loss, graceful drain, multi-worker status totals, and the HTTP-required empty wire body plus exact representation headers for `HEAD`.

The status response and every returned score item carry one `scoring_identity`. PangoPup hashes the RFC 8785 canonical `pangopup.active-scoring-identity.v1` preimage over software version, admitted runtime-profile identity, and effective CPU policy. Precomputed, modeled, cached, ambiguous, mixed, and mixed-batch rejected items all carry the same value. Request-level errors have no result item. Detailed route provenance stays unchanged, and standalone CLI output does not gain this service-only field.

## Request contract schema

The status response also carries `request_contract`. This machine-readable object reports the `/v1/score` API version, media type, body and item limits, uncached model-work units, assembly, model allele and exact-edit limits, all accepted variant and gene forms, and every accepted primary-contig spelling. Clients should consume this object instead of copying those values from prose. It stays identical across readiness and queue states. It contains no host or request details and does not enter `scoring_identity`.

Each object contains exactly the properties listed in this public nested schema:

```text
request_contract: object
|-- api_version: string
|-- route: string
|-- content_type: string
|-- max_body_bytes: integer
|-- variants: object
|   |-- min_items: integer
|   |-- max_items: integer
|   |-- max_uncached_model_items: integer
|   |-- model_work_unit: string
|   |-- assembly: string
|   |-- max_model_allele_bases: integer
|   |-- max_exact_edit_sequence_bases: integer
|   |-- forms: array of strings
|   `-- contigs: array of objects
|       |-- canonical: string
|       `-- accepted: array of strings
|-- gene_filter: object
|   |-- accepted_forms: array of strings
|   |-- version_minimum: integer
|   |-- version_maximum: integer
|   `-- version_allows_leading_zero: boolean
`-- model_only: object
    |-- type: string
    `-- optional: boolean
```

Array entries and scalar values carry the enforced strings, numbers, and booleans reported by the running service. The exact contract test below pins the full object.

The status `model` object reports `planning_millis_per_unit: 10241` from the slowest retained p50 and `full_capacity_planning_seconds` as `ceil(queue_capacity × planning_millis_per_unit / 1000)` with a one-second minimum. The default capacity reports 205 seconds. The same factor and upward-rounded arithmetic produce `Retry-After` from admitted units. Worker count does not divide either value. These retained measurements provide planning guidance. They do not guarantee latency or recommend a client timeout. Strict JSON status consumers must adopt both additive fields before deployment.

```bash
cargo test --locked --quiet --package pangopup-assets active_identity >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup scoring_identity >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup request_contract >/dev/null 2>&1
printf 'status and every returned HTTP item share one canonical active scoring identity\n' | mustmatch like 'status and every returned HTTP item share one canonical active scoring identity'
```

## Cache-dependent model-work limit

The uncached-model limit counts distinct canonical cache keys that remain misses after the live cache lookup. It does not count submitted items or elapsed time.

An identical refused request can be accepted later after another successful operation populates enough keys, but cache writes are best-effort, entries can be evicted, and eventual success is not guaranteed.

A batch within the item limit is accepted when its variants need no uncached model work, regardless of the uncached-model ceiling.

`MODEL_BATCH_TOO_LARGE` returns HTTP 422 without `Retry-After`. Time alone does not change cache state, so clients should not retry an unchanged request blindly.

Without evidence of cache warming, split the batch or reduce its distinct uncached model work to the reported `max_uncached_model_items`.

## Scoring requests

```bash
cache_contract=$(awk '/^## Cache-dependent model-work limit$/ { on=1; next } on && (/^## / || /^```/) { exit } on' http-service.md)
for statement in \
  'The uncached-model limit counts distinct canonical cache keys that remain misses after the live cache lookup.' \
  'It does not count submitted items or elapsed time.' \
  'An identical refused request can be accepted later after another successful operation populates enough keys, but cache writes are best-effort, entries can be evicted, and eventual success is not guaranteed.' \
  'A batch within the item limit is accepted when its variants need no uncached model work, regardless of the uncached-model ceiling.' \
  '`MODEL_BATCH_TOO_LARGE` returns HTTP 422 without `Retry-After`. Time alone does not change cache state, so clients should not retry an unchanged request blindly.' \
  'Without evidence of cache warming, split the batch or reduce its distinct uncached model work to the reported `max_uncached_model_items`.'; do
  printf '%s' "$cache_contract" | rg -F -- "$statement" >/dev/null
done
printf 'the cache-dependent model-work refusal contract is complete\n' | mustmatch like 'the cache-dependent model-work refusal contract is complete'
```

The scoring route requires exactly one parsed `application/json` content type. Case does not matter and legal parameters are accepted. Missing, malformed, non-JSON, JSON suffix, and repeated values receive HTTP 415 with `UNSUPPORTED_MEDIA_TYPE`. This validation follows route and method selection. It precedes readiness checks and body reads. The real executable test sends these headers through the HTTP listener and pins the response.

The optional `gene` filter accepts a stable Ensembl identifier, a versioned GENCODE identifier, or a versioned GENCODE identifier ending in `_PAR_Y`. The route normalizes every accepted form to the stable gene before lookup or model-result filtering. Every structured score record reports the source identity under `gene` and the same stable grouping and filtering identity under `stable_gene`. Precomputed records report the stable identifier in both fields. Model records preserve the exact GENCODE version and `_PAR_Y` identity under `gene`. Consumers must retain `gene` when exact version or PAR identity matters. Adapter tests submit the model-reported versioned forms and reject zero, leading-zero, missing, overflowing, repeated, and unknown suffixes with HTTP 400 and `INVALID_REQUEST`.

```bash
cargo test --locked --quiet --package pangopup-cli http_gene_filter_accepts_reported_identity_and_matches_its_stable_gene >/dev/null 2>&1
printf 'HTTP accepts a reported versioned gene filter and matches its stable gene\n' | mustmatch like 'HTTP accepts a reported versioned gene filter and matches its stable gene'
```

The public route returns HTTP 200 with one ordered item outcome whenever the scoring envelope and shared options are valid and no operational failure occurs. This rule covers singleton, mixed, and all-rejected batches. HTTP success reports that PangoPup classified the batch. Each item reports whether annotation succeeded. Every item carries the exact submitted variant string under `input`. Contig aliases, RefSeq accessions, and exact edits remain unchanged there while their genomic fields use the existing normalized representation. Equivalent uncached model inputs in one request share one model-work admission unit and one model execution. They remain separate ordered items with their exact `input` values. Clients must still validate count, membership, duplicates, and response shape. An invalid variant string has `status: "rejected"`, empty `records` and `source_reference_ambiguities`, no provenance, and the stable generic `INVALID_VARIANT` error. A normalized variant that the model rejects keeps its normalized genomic fields and the existing generic `MODEL_REJECTED` error. Every rejected item adds one stable `reason` slug. Existing status, code, and message fields keep their meanings. The miniature installed profile exercises both forms through the real executable and HTTP listener.

The current rejection-reason vocabulary is closed: `malformed_variant` identifies a malformed literal or envelope-valid variant string; `unsupported_genomic_value` identifies an unsupported assembly, contig, coordinate, or allele value; `invalid_exact_edit_geometry` identifies an invalid or unusable exact edit; `unsupported_variant_shape` identifies a normalized form the model cannot score; `allele_too_long` identifies alleles above the reported model limit; `reference_context_unavailable` identifies a position without the required GRCh38 window; `reference_mismatch` identifies a submitted reference allele or deletion sequence that disagrees with GRCh38; `unsupported_reference_symbol` identifies a reference window symbol the model cannot score; `not_in_annotated_gene` identifies a position outside every GENCODE gene; and `other_model_rejection` safely classifies a model rejection introduced by a newer engine before the HTTP vocabulary adds a specific reason. Reason values never contain coordinates, sequences, symbols, offsets, paths, provider details, or blame. Clients must handle unknown future reason slugs as rejected outcomes.

Each `variants[]` value also accepts `GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE` and `GRCh38:CONTIG:DEL:START:END:SEQUENCE`. Coordinates are one-based. Insertion coordinates must be adjacent. A deletion interval is inclusive, must not start at one, and must have the same length as its submitted sequence. Sequences contain 1–99 uppercase A/C/G/T bases. PangoPup reads the left anchor from its installed GRCh38 reference before routing, caching, queue admission, and inference. Equivalent exact and anchored inputs produce the same canonical response allele and cache identity. A deleted-sequence mismatch becomes the existing normalized item rejection when the left anchor is valid. Boundary and anchor failures become invalid item outcomes. Reference corruption remains a request-level server failure.

```bash
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle real_executable_ \
  >/dev/null 2>&1
printf 'a valid batch returns HTTP 200 with one ordered outcome for every valid or rejected item\n' | mustmatch like 'a valid batch returns HTTP 200 with one ordered outcome for every valid or rejected item'
```

Backend scoring and unusable-cache failures invalidate the complete request and remain HTTP 500. Their machine-readable `error.code` remains `MODEL_SCORING` or `MODEL_CACHE_INVALID`. Worker loss and service readiness failures also remain request-level errors. Inside-out service tests inject all backend families and pin their exact status and generic response body. The public fixture does not corrupt a production-only model or cache path solely to manufacture those server failures.

## What a consumer pins

A consumer stores one value beside a retained score and compares it later. That value must move whenever a score can change. It must hold still otherwise. `scoring_identity` fails the second half. It hashes the effective CPU policy of the deployment. Scaling a service from one thread to four moves it. Measurement on one host and one build found every answer unchanged. [`architecture/compatibility.md`](../architecture/compatibility.md) states the strength of that measurement and tells a reader when to repeat it.

The status response publishes three further values. `data_set_version` is the value to store where a system has one version field. PangoPup hashes the RFC 8785 canonical `pangopup.scoring-data-set-version.v1` preimage over the software version and the runtime profile identity. `runtime_profile_id` is the SHA-256 of the admitted canonical runtime profile. It covers every asset digest and every scoring input the profile declares. It does not cover the software version. A PangoPup version change can move an answer with every asset unchanged. A consumer therefore stores `data_set_version` rather than `runtime_profile_id`. `scoring_semantics` names the score contract both routes answer under.

`--model-workers` and `--model-threads` move neither `data_set_version` nor `runtime_profile_id`. They do move `model.effective_cpu_policy`. `scoring_identity` moves with that policy.

The runtime profile declares its own `cpu_policy`. That value describes the assets PangoPup qualified. It never describes the running service. `production_runtime_profile` fixes it at `sequential:1/1` and admits an installed profile only where it matches. A service started with `--model-threads 4` reports `sequential:4/1` under `model.effective_cpu_policy` while its profile still declares `sequential:1/1`. Read the profile value as a property of the assets. Read the status value as a property of the deployment.

The two scoring routes do not carry provenance to the same depth. [`architecture/compatibility.md`](../architecture/compatibility.md) states which facts a precomputed item leaves out and where a consumer reads them instead. The test below pins both field sets, so that statement cannot drift away from the response.

```bash
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle pinned_values_hold_across_cpu_policies_and_move_with_the_scored_inputs \
  2>/dev/null | rg -F '1 passed; 0 failed' >/dev/null
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle status_publishes_a_recomputable_data_set_version \
  2>/dev/null | rg -F '1 passed; 0 failed' >/dev/null
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle each_route_reports_the_provenance_its_answer_used \
  2>/dev/null | rg -F '1 passed; 0 failed' >/dev/null
cargo test --locked --quiet --package pangopup-assets --lib \
  every_leaf_fact_changes_the_profile_identity \
  2>/dev/null | rg -F '1 passed; 0 failed' >/dev/null
printf 'the pinned values hold across CPU policies and move with every scoring input\n' | mustmatch like 'the pinned values hold across CPU policies and move with every scoring input'
```

A consumer reads what to pin from two documents rather than from a source line. `README.md` states it beside the status route it describes. `architecture/compatibility.md` carries the full contract under `## What a consumer pins`.

```bash
readme=$(cat ../README.md)
for statement in \
  'Store `data_set_version` as the data-set version when a system has one version field.' \
  'A worker or thread change moves `scoring_identity` and never moves `data_set_version`.'; do
  printf '%s' "$readme" | rg -F -- "$statement" >/dev/null
done
! printf '%s' "$readme" | rg -F -- 'Store this identity as the data-set version' >/dev/null
pinning=$(awk '/^## What a consumer pins$/ { on=1; next } on && /^## / { exit } on' ../architecture/compatibility.md)
for statement in \
  'Store `data_set_version` as the data-set version when a system has one version field.' \
  '`data_set_version` hashes the PangoPup version and the runtime profile identity.' \
  '`runtime_profile_id` hashes the whole admitted runtime profile.' \
  'It covers every asset digest and every scoring input, `assembly`, `semantics`, `distance`, `masking_policy` and `cpu_policy` among them.' \
  'No worker or thread setting moves `data_set_version` or `runtime_profile_id`.' \
  '`runtime_profile_id` alone is not the value to store. A PangoPup version change can move an answer with every asset unchanged. No such change moves `runtime_profile_id`. `data_set_version` covers the version too.' \
  '`scoring_identity` also hashes the effective CPU policy. A thread change moves it. Measurement on one host and one build found no score that a thread change moved.' \
  'The runtime profile declares `cpu_policy` for the assets PangoPup qualified. The status `model.effective_cpu_policy` reports what the running service uses.' \
  'A service started with `--model-threads 4` reports `sequential:4/1` as its effective policy while its runtime profile still declares `sequential:1/1`.' \
  'A precomputed score item carries six provenance fields and a modeled score item carries twelve.' \
  'A precomputed value ran no model, read no reference window and applied no runtime mask. `model_bundle_id`, `model_profile`, `effective_cpu_policy`, `reference_bundle_id`, `reference_profile`, `reference_sequence_set_sha256`, `mask_bytes` and `mask_sha256` describe none of it.' \
  'A precomputed item still reports `masked` and `window`. Both values come from the published dataset manifest.' \
  '`bundle_id`, `source_doi` and `source_archive_md5` pin the published dataset a precomputed value came from.' \
  'A modeled score item carries none of those three fields.' \
  'Both routes answer under one scoring semantics. The status response reports it as `scoring_semantics`.'; do
  printf '%s' "$pinning" | rg -F -- "$statement" >/dev/null
done
identity=$(awk '/^## Active scoring identity$/ { on=1; next } on && /^## / { exit } on' ../architecture/service.md)
printf '%s' "$identity" | rg -F -- '`data_set_version` carries the same inputs without the effective CPU policy.' >/dev/null
printf '%s' "$identity" | rg -F -- 'The installed naming source stays outside the preimage.' >/dev/null
! printf '%s' "$identity" | rg -F -- 'active policy that can change an answer' >/dev/null
runtime_data=$(cat ../architecture/runtime-data.md)
printf '%s' "$runtime_data" | rg -F -- '`data_set_version` gives a consumer one concise version value that no deployment setting moves.' >/dev/null
! printf '%s' "$runtime_data" | rg -F -- 'This environment identity gives a consumer one concise version value.' >/dev/null
printf 'a consumer can cite a document for what to pin\n' | mustmatch like 'a consumer can cite a document for what to pin'
```
