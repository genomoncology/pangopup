# Nobody has measured Pangolin on anything but a CPU core

Status: open

## Observation

Every indel decision in
[`2026-09-11-indel-model-cost-and-prefilter-soundness.md`](2026-09-11-indel-model-cost-and-prefilter-soundness.md)
turns on one number this project does not have: Pangolin inferences per hour on
an accelerator, with real batching.

The only measured figure is the retained CPU one. An uncached inference takes
about 4.3 seconds and the subsequent cache hit takes about 0.7 milliseconds. The
kernel runs ONNX on the CPU by design
([ADR 0014](../../architecture/decisions/0014-authenticated-onnx-cpu-kernel.md)).

Without a throughput number, the precompute options differ by four orders of
magnitude and none of them can be costed.

## Why it matters

Two candidate precompute scopes, from the indel issue:

**Exhaustive bounded class.** All one-base insertions and one-to-four-base
deletions inside genes, the scope Illumina chose for the SpliceAI precomputed
files. That is eight records per locus against the current three. Over
1,366,418,555 loci it is 10,931,348,440 model evaluations.

**Observed indels only.** The indels gnomAD and ClinVar actually report inside
gene bodies. The count has not been extracted and is expected to be tens of
millions.

At an unknown rate, the first is a compute programme and the second is a
weekend. At a known rate, both are multiplication.

A miss on an observed-only index is not a lost answer. The variant falls through
to the model at 4.3 seconds once, and the SQLite cache
([ADR 0019](../../architecture/decisions/0019-persistent-model-result-cache.md))
keeps it. Only speed is at stake.

## The Apple silicon question

Ian has an Apple silicon Mac available. Two routes exist: PyTorch with the MPS
backend, or ONNX Runtime with the CoreML execution provider. The ONNX model is
already in hand, so CoreML needs no conversion step.

Pangolin is a stack of dilated residual convolutions. Dilated convolution
support is incomplete on both backends. An unsupported operator does not fail.
MPS falls back to the CPU for that operator and CoreML returns the subgraph to
the CPU. The run produces correct output and no speedup, and nothing reports it.

Any measurement on this machine must therefore first establish that the GPU is
executing the model. Confirm device placement per operator, or confirm a
GPU-utilization reading during the run, before recording any rate.

The Mac is worth measuring first because it costs nothing and it is already
here. It may also be enough to run the observed-only job outright.

## Renting, if the Mac cannot carry it

The workload is a batch job in a container with no long-lived state. Modal fits
that shape: per-second billing, no cluster to maintain, and a killed job costs
nothing further. RunPod is cheaper per GPU-hour and hands over machine
management. Vast.ai is cheaper again and less reliable. Colab suits a one-hour
measurement and does not suit a long batch run, because a disconnect loses it.

None of these has been evaluated against this workload. They are named as
starting points, not as a selection.

## Licensing

Pangolin is GPL. Running it to produce scores is not distribution of it, and the
scores are not a derived work of the software under the GPL. Renting a machine
to run it changes nothing about that. The process boundary the product already
maintains is unaffected.

The separate trap from the indel issue still stands. SpliceAI's precomputed
scores are CC BY-NC 4.0 and must not become an oracle, a seed, or a validation
set for anything shipped.

## The measurement

One session, on the Mac, then on one rented instance if the Mac falls short.

- Confirm the GPU actually executes the model, by device placement or by
  utilization, and record how that was confirmed.
- Report inferences per hour at several batch sizes, since batching is the whole
  question and a batch of one answers nothing.
- Report the CPU baseline on the same host in the same run, so the speedup is a
  ratio measured once rather than a comparison across hosts.
- Confirm the accelerator output matches the CPU output. Floating-point results
  differ between execution providers, and a precompute corpus that disagrees
  with the shipped CPU kernel is not usable. State the observed difference
  rather than asserting there is none.

## What a ticket would need

- The measured rate, and the host, backend, batch size and model build it was
  measured on.
- The extracted count of observed genic indels in gnomAD and ClinVar, so the
  observed-only scope has a real denominator.
- Confirmed terms of use for every source, per the indel issue.
- The accelerator-versus-CPU agreement result, with a stated tolerance.
- A decision on scope, exhaustive or observed-only, made after the rate is known
  and not before.
