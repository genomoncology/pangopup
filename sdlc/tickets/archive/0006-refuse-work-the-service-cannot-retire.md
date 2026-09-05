---
flow: build
priority: 5
---
# A saturated service answers the caller instead of holding it

The service admits far more model work than it can retire, so callers reach
their own timeout rather than the documented full-queue answer.

Admission counts requests in flight. It does not account for how much model work
each request carries, and a request may carry up to ten uncached variants. At
the shipped defaults the service can hold roughly 160 queued inferences against
a single sequential worker.

Model variants do not cost the same. The retained per-variant p50s for the
shipped `1/1` singleton policy, in `planning/artifacts/022-reference-alternate-batching.md`,
range from 4.141 to 10.241 seconds across the six measured cases. A queue length
therefore buys a planning estimate and never a guaranteed retirement time; the
same queue holds two and a half times as long when it fills with expensive
variants. Taking the slowest measured case, the last caller admitted at the
shipped defaults waits something near twenty-seven minutes, and nothing is
refused before then.

Measured on that host. Twelve concurrent requests of five uncached variants each,
default settings:

```
elapsed 300.1s    served 10    client timeouts 2    refused 0
```

The same load against a service configured with the smallest queue:

```
elapsed 9.2s      served 2     refused 8    refusal latency 0.2s
```

The refusal path works. The default bound is the problem: it is expressed in
callers, and the cost it needs to bound is model work.

`README.md` offers the full-queue status as the signal that the service is
saturated. At the shipped defaults a caller times out long before it sees one,
then retries, and the retry queues behind the work it just abandoned.

Three choices this ticket settles, because the agent cannot make them alone.

The admission bound must account for the model work a request carries rather
than the number of callers holding one, and it must count the work already
running as well as the work waiting.

The default capacity is 20 admitted uncached-model-variant units across running and waiting work. The slowest retained p50 measurement is 10.241 seconds per variant. One sequential worker therefore gives the last admitted unit a planning estimate of about 205 seconds. This is closer to three and a half minutes than three minutes. The earlier three-minute wording cannot admit two complete ten-variant requests under the retained measurement, so the measured arithmetic controls. This estimate is not a worst-case latency promise.

The default uses the slowest retained per-variant measurement rather than the median. A cheaper workload will shed sooner than strictly necessary. That is the accepted cost. A default based on the median would hold expensive work past the stated estimate. A request-count bound would recreate the current defect.

`--model-queue-capacity` retains its public name but now sets this total admitted-work bound in uncached-model-variant units. The flag still accepts 1 through 1024. A configured value below ten may refuse a valid maximum-size request; the documented default of 20 admits one whenever fewer than eleven units are already admitted. The service keeps whole requests and FIFO order. It does not split or reorder a request to fill unused capacity.

The wait is an estimate and must be described as one wherever it is written
down. Per-variant cost varies by more than a factor of two, so no bound
expressed in queued work can promise a retirement time.

Issue: `sdlc/issues/2026-09-04-shed-model-load-before-clients-time-out.md`

Done, observably:

- A caller whose work would take the admitted total above the configured bound receives the full-queue answer immediately. At the default, the bound corresponds to the documented slowest-retained-p50 planning estimate.
- Running plus waiting uncached model variants never exceeds the configured capacity. The bound does not change when the same work arrives spread across more callers or gathered into fewer.
- A single request carrying the documented maximum of uncached variants is still
  served at default settings.
- Authoritative index hits and completed SQLite hits consume no admission units.
- Cancellation, worker completion, worker loss, send failure, and shutdown release or preserve the exact units required by the existing lifecycle contract.
- `README.md`, command help, `architecture/service.md`, and ADR 0025 state the 20-unit default, the roughly 205-second one-worker planning estimate, its slowest retained p50 source, its non-guarantee, and the configuration lever for a different workload.
- The status route reports `running`, `queued`, and `queue_capacity` in uncached-model-variant units and adds `work_unit: "uncached_model_variant"`. Operators compare `running + queued` with `queue_capacity` directly.
- Tests prove equal admission for equal model work across different request groupings, exact-boundary admission, immediate 429 above the bound, maximum-size default admission, bypass behavior, lifecycle accounting, and configured multi-worker concurrency while status reports unit totals. At least one saturation case fails before the change.

Boundary: do not change the per-request maximum of uncached variants, the refusal status or code already returned when the capacity is full, the existing status field names, the worker and thread defaults, or the measured scheduling recorded in ADR 0024. Do not add a timeout, deadline, retry, request splitting, prioritization, or partial admission. Requests that are admitted are served exactly as they are today. Ticket 0013 owns retry guidance.
