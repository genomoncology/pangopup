# Shed model load before clients time out

## Observation

`Dispatcher::admit` in `crates/pangopup-cli/src/service.rs:215` counts one queue slot per request. Each request carries up to 10 uncached model variants. The default capacity is 16 (line 408) and the default worker shape is one sequential worker.

A full default queue therefore holds up to 160 inferences. At the measured ~5 s per uncached variant on the retained Ryzen host, the last admitted request waits roughly 13 minutes. HTTP 429 is returned only after that.

Measured on this host, twelve concurrent requests of five uncached variants each against a default service:

```
elapsed 300.1s
status counts: 200 x10, client timeout x2
429 returned: none
```

The same test against `--model-queue-capacity 1 --model-workers 1`:

```
elapsed 9.2s
status counts: 429 x8, 200 x2
latencies: 0.2s x8, 4.6s, 9.2s
```

Backpressure works. The default admits far more work than it can retire, so clients time out instead of being turned away.

## Why this matters

The README offers 429 as the documented full-queue signal. At default settings a caller reaches its own timeout first and learns nothing. Retried work then lands behind the queue it just abandoned.

## Suggested direction

Count admitted variants rather than admitted requests, so the bound describes the work rather than the number of callers. Then set the default from the measured per-variant cost and a stated worst-case wait. Both are decisions worth recording: the current default is not obviously wrong, it is untied to the time a client is willing to wait.
