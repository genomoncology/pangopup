# Foreground HTTP service

The executable exposes one foreground service command with explicit bounded
model capacity. Invalid capacity fails before opening assets or binding a
listener.

```bash
pangopup serve --help | mustmatch like 'Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]

Run the foreground HTTP scoring service.'
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

The inside-out HTTP tests inject miniature providers and exercise the actual
router without downloading or running the production model. They pin exact
success/error bytes, lookup and SQLite bypass under saturation, FIFO admission,
429 backpressure, disconnect behavior, worker loss, graceful drain, and the
HTTP-required empty wire body plus exact representation headers for `HEAD`.

Worker backend failures return HTTP 500 with the generic `scoring failed`
message. Their machine-readable `error.code` retains the backend family:
`MODEL_REJECTED`, `MODEL_SCORING`, or `MODEL_CACHE_INVALID`.
