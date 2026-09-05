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

The inside-out HTTP tests inject miniature providers and exercise the actual router without downloading or running the production model. They pin exact success/error bytes, lookup and SQLite bypass under saturation, whole-request FIFO admission by uncached model variant, exact-boundary 429 backpressure with retry guidance, overweight-request refusal without retry guidance, disconnect accounting, worker loss, graceful drain, multi-worker status totals, and the HTTP-required empty wire body plus exact representation headers for `HEAD`.

The scoring route requires exactly one parsed `application/json` content type. Case does not matter and legal parameters are accepted. Missing, malformed, non-JSON, JSON suffix, and repeated values receive HTTP 415 with `UNSUPPORTED_MEDIA_TYPE`. This validation follows route and method selection. It precedes readiness checks and body reads. The real executable test sends these headers through the HTTP listener and pins the response.

The public route identifies an entirely model-rejected request as a client error and keeps the generic `scoring failed` message. A request with at least one normal outcome and no operational failure returns HTTP 200. The response preserves one ordered outcome for every input. An item that the model rejects has `status: "rejected"`, empty `records` and `source_reference_ambiguities`, no provenance, and the stable generic `MODEL_REJECTED` error. HTTP 422 applies only when every input outcome is model-rejected. The miniature installed profile exercises both behaviors through the real executable and HTTP listener.

```bash
cargo test --locked --quiet --package pangopup-cli --features service-test-fixtures \
  --test http_service_lifecycle real_executable_ \
  >/dev/null 2>&1
printf 'MODEL_REJECTED is HTTP 422 for an unusable request and an ordered item outcome in a mixed response\n' | mustmatch like 'MODEL_REJECTED is HTTP 422 for an unusable request and an ordered item outcome in a mixed response'
```

Backend scoring and unusable-cache failures invalidate the complete request and remain HTTP 500. Their machine-readable `error.code` remains `MODEL_SCORING` or `MODEL_CACHE_INVALID`. Worker loss and service readiness failures also remain request-level errors. Inside-out service tests inject all backend families and pin their exact status and generic response body. The public fixture does not corrupt a production-only model or cache path solely to manufacture those server failures.
