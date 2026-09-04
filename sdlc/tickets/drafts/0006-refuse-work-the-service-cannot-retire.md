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

The default must be derived from the measured per-variant cost against a stated
target wait, rather than chosen as a bare number. Target roughly three minutes,
and derive it from the slowest retained per-variant measurement rather than the
median, so the estimate degrades toward shedding early rather than toward
holding callers. This leaves room for more than one full request while keeping a
caller inside a normal client timeout. Deriving from the slowest case means a
queue of cheap variants sheds sooner than it strictly must; that is the intended
direction of the error.

The wait is an estimate and must be described as one wherever it is written
down. Per-variant cost varies by more than a factor of two, so no bound
expressed in queued work can promise a retirement time.

Issue: `sdlc/issues/2026-09-04-shed-model-load-before-clients-time-out.md`

Done, observably:

- A caller whose work the service cannot retire within the stated wait receives
  the full-queue answer rather than being held.
- The admitted amount of model work is bounded, and the bound does not change
  when the same work arrives spread across more callers or gathered into fewer.
- A single request carrying the documented maximum of uncached variants is still
  served at default settings.
- The wait the default targets is written where an operator setting up the
  service will find it, described as an estimate, together with what to change
  when their own workload differs.
- The status route reports the admitted and waiting amounts in the same unit as
  the capacity it reports beside them, so an operator can compare them without
  converting.
- The suite pins the saturated answer with a case that fails before the change.

Boundary: do not change the per-request maximum of uncached variants, the
refusal status or code already returned when the queue is full, the rest of the
status route's fields, the worker and
thread defaults, or the measured scheduling recorded in ADR 0024. Do not add a
timeout, a deadline, or a retry to the service. Requests that are admitted are
served exactly as they are today.
