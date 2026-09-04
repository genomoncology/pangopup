# Return a client status when the model rejects the input

## Observation

The HTTP service answers HTTP 500 for input the model rejects. The CLI treats the same input as a request error and exits 2.

```
CLI : {"status":"error","code":"MODEL_REJECTED","message":"model alleles exceed 100 bases (REF 1, ALT 151)"}   exit 2
HTTP: 500 {"error":{"code":"MODEL_REJECTED","message":"scoring failed"}}
```

Both reproduce against an installed profile on `v0.3.0` code at `bfe1c8c`:

```bash
pangopup lookup --variant "GRCh38:chr12:6801303:G:G$(python3 -c 'print("A"*150)')"
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  -d '{"variants":["GRCh38:chr12:6801305:G:GA"]}'
```

A REF that does not match GRCh38 and an allele over 100 bases both answer 500. A malformed variant string correctly answers 400 `INVALID_REQUEST`, so the boundary is inconsistent within the same route.

## Why this matters

`MODEL_REJECTED` names a request the service will never accept. A 5xx status tells every intermediary the opposite. Proxies and client libraries retry 5xx, so a submitted typo becomes repeated inference load. Error-rate monitoring and any availability target counts these as service failures.

Ticket 0001 preserved the backend failure code. The status code did not follow.

## Suggested direction

Map `MODEL_REJECTED` to a 4xx status and keep `MODEL_SCORING` and `MODEL_CACHE_INVALID` at 500. Decide whether the reply carries the specific message; the CLI already exposes it, so withholding it over HTTP buys no confidentiality.
