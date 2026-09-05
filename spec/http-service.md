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

The inside-out HTTP tests inject miniature providers and exercise the actual router without downloading or running the production model. They pin exact success/error bytes, lookup and SQLite bypass under saturation, whole-request FIFO admission by uncached model variant, exact-boundary 429 backpressure with retry guidance, permanent HTTP 422 rejection above the reported request limit, disconnect accounting, worker loss, graceful drain, multi-worker status totals, and the HTTP-required empty wire body plus exact representation headers for `HEAD`.

The status response and every returned score item carry one `scoring_identity`. PangoPup hashes the RFC 8785 canonical `pangopup.active-scoring-identity.v1` preimage over software version, admitted runtime-profile identity, and effective CPU policy. Precomputed, modeled, cached, ambiguous, mixed, and mixed-batch rejected items all carry the same value. Request-level errors have no result item. Detailed route provenance stays unchanged, and standalone CLI output does not gain this service-only field.

The status response also carries `request_contract`. This machine-readable object reports the `/v1/score` API version, media type, body and item limits, uncached model-work units, assembly, model allele and exact-edit limits, all accepted variant and gene forms, and every accepted primary-contig spelling. Clients should consume this object instead of copying those values from prose. It stays identical across readiness and queue states. It contains no host or request details and does not enter `scoring_identity`.

```bash
cargo test --locked --quiet --package pangopup-assets active_identity >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup scoring_identity >/dev/null 2>&1
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures --bin pangopup request_contract >/dev/null 2>&1
printf 'status and every returned HTTP item share one canonical active scoring identity\n' | mustmatch like 'status and every returned HTTP item share one canonical active scoring identity'
```

The scoring route requires exactly one parsed `application/json` content type. Case does not matter and legal parameters are accepted. Missing, malformed, non-JSON, JSON suffix, and repeated values receive HTTP 415 with `UNSUPPORTED_MEDIA_TYPE`. This validation follows route and method selection. It precedes readiness checks and body reads. The real executable test sends these headers through the HTTP listener and pins the response.

The optional `gene` filter accepts a stable Ensembl identifier, a versioned GENCODE identifier, or a versioned GENCODE identifier ending in `_PAR_Y`. The route normalizes every accepted form to the stable gene before lookup or model-result filtering. It does not change the gene written in a result. Adapter tests submit the model-reported versioned forms and reject zero, leading-zero, missing, overflowing, repeated, and unknown suffixes with HTTP 400 and `INVALID_REQUEST`.

```bash
cargo test --locked --quiet --package pangopup-cli http_gene_filter_accepts_reported_identity_and_matches_its_stable_gene >/dev/null 2>&1
printf 'HTTP accepts a reported versioned gene filter and matches its stable gene\n' | mustmatch like 'HTTP accepts a reported versioned gene filter and matches its stable gene'
```

The public route returns HTTP 200 with one ordered item outcome whenever the scoring envelope and shared options are valid and no operational failure occurs. This rule covers singleton, mixed, and all-rejected batches. HTTP success reports that PangoPup classified the batch. Each item reports whether annotation succeeded. Every item carries the exact submitted variant string under `input`. Contig aliases, RefSeq accessions, and exact edits remain unchanged there while their genomic fields use the existing normalized representation. Duplicate inputs remain separate ordered items with the same `input` value. Clients must still validate count, membership, duplicates, and response shape. An invalid variant string has `status: "rejected"`, empty `records` and `source_reference_ambiguities`, no provenance, and the stable generic `INVALID_VARIANT` error. A normalized variant that the model rejects keeps its normalized genomic fields and the existing generic `MODEL_REJECTED` error. The miniature installed profile exercises both forms through the real executable and HTTP listener.

Each `variants[]` value also accepts `GRCh38:CONTIG:INS:LEFT:RIGHT:SEQUENCE` and `GRCh38:CONTIG:DEL:START:END:SEQUENCE`. Coordinates are one-based. Insertion coordinates must be adjacent. A deletion interval is inclusive, must not start at one, and must have the same length as its submitted sequence. Sequences contain 1–99 uppercase A/C/G/T bases. PangoPup reads the left anchor from its installed GRCh38 reference before routing, caching, queue admission, and inference. Equivalent exact and anchored inputs produce the same canonical response allele and cache identity. A deleted-sequence mismatch becomes the existing normalized item rejection when the left anchor is valid. Boundary and anchor failures become invalid item outcomes. Reference corruption remains a request-level server failure.

```bash
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle real_executable_ \
  >/dev/null 2>&1
printf 'a valid batch returns HTTP 200 with one ordered outcome for every valid or rejected item\n' | mustmatch like 'a valid batch returns HTTP 200 with one ordered outcome for every valid or rejected item'
```

Backend scoring and unusable-cache failures invalidate the complete request and remain HTTP 500. Their machine-readable `error.code` remains `MODEL_SCORING` or `MODEL_CACHE_INVALID`. Worker loss and service readiness failures also remain request-level errors. Inside-out service tests inject all backend families and pin their exact status and generic response body. The public fixture does not corrupt a production-only model or cache path solely to manufacture those server failures.
