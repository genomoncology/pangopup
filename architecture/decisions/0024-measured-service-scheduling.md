# 0024 — Host-qualified service model partition

Status: accepted

## Decision

Retain `sequential:1/1` as Pangopup's portable ordinary model policy. On the
measured AMD Ryzen 7 5825U with one logical CPU from each physical core,
qualify `1×1`, `1×2`, and `1×4` for matching one-, two-, and four-core budgets,
and two four-thread sessions (`2×4`) for the eight-core budget.

Do not infer a mapping from logical CPU count, `available_parallelism`, cgroup
quota, or an unmatched host identity. A later service may consume the retained
host-qualified table only after it proves the exact CPU and affinity boundary;
otherwise it uses the portable `1×1` behavior from ADR 0017.

## Why

Thirty fresh-process rounds compared every integer worker/thread partition of
1, 2, 4, and 8 physical cores. All candidates were exact and below 1 GiB RSS.
Most multi-session candidates exceeded the reviewed 125-percent single-request
latency guard. At eight cores, `2×4` remained inside the guard and beat `1×8`
on model-batch throughput, so the mechanical rules selected it.

Warm 1/10/100-SNV mmap batches remained fast while a model batch was in flight. This
supports the durable requirement that the service keep lookup outside model
admission; it is not itself proof of a production queue or scheduler.

## Boundary

The retained dispatcher is ignored integration-test code. This decision does
not select queue capacity, backpressure, dispatch, cache connection ownership,
fill coalescing, HTTP framework, resource limits, or a portable session count.
Those remain service-layer work. Exact measurements and the 14-case selected
reruns are retained in the Ticket 040 report and raw JSON Lines artifact.
