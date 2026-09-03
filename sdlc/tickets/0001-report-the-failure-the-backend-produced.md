---
flow: build
priority: 6
---
# The HTTP service reports the failure family the backend produced

The `lookup` command and the HTTP service run the same worker and see the same
backend error, and they report it differently.

`crates/pangopup-cli/src/main.rs` maps a backend error into a `Failure` that
carries a code. Three codes reach the worker today: `MODEL_REJECTED` with exit
2, `MODEL_SCORING` with exit 1, and `MODEL_CACHE_INVALID` with exit 1. The CLI
prints whichever one occurred.

`process_job` in `crates/pangopup-cli/src/service.rs` discards that code with
`.map_err(|_| WorkerReply::ScoringFailed)`. Every one of the three becomes HTTP
500 `SCORING_FAILED`. A caller over HTTP cannot tell a rejected request from a
scoring failure, and cannot tell either from a corrupt cache.

A downstream service that scores variants through this HTTP surface has to
choose between two wrong behaviours when it sees `SCORING_FAILED`. It can treat
every one as a transient fault and retry a request that will never succeed, or
it can treat every one as a determinate answer and record an absence that the
model never asserted. Neither is honest. That consumer has already reported
this.

Done, observably:

- An HTTP caller receiving a failure can tell which of the three backend failure
  families occurred, from the response alone.
- For the same input, the HTTP surface and the `lookup` command agree on which
  failure family occurred.
- A rejected request and a scoring failure are distinguishable without reading
  a free-text message.
- The spec suite pins each family with a case that fails before the change.

Boundary: do not change what the backend decides, what `ModelFallbackError`
carries, or how the CLI reports. Do not add a new failure family. Do not change
the success path or the response shape of a successful score. The batch and
worker dispatch behaviour stays as it is; only the failure a caller sees
changes.
