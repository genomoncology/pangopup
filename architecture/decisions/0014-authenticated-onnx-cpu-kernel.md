# 0014 — Authenticated ONNX CPU model kernel

Status: accepted

## Decision

Represent the twelve pinned Pangolin checkpoints as one combined ONNX graph and
execute it through `ort` 2.0.0-rc.12 / ONNX Runtime 1.24.2 on the default CPU
execution provider. The first baseline uses one mutable session, sequential
execution, graph optimization level `All`, and intra/inter-op thread counts
`1/1`.

`pangopup-model` owns a deliberately private raw-kernel boundary. It accepts a
10,001–10,200-base A/C/G/T/N context plus strand and returns twelve selected raw
score channels in genomic orientation. It does not construct genomic variants,
average replicates, reconcile reference and alternate arrays, mask scores,
select extrema, or render a public Pangolin result.

The runtime artifact is a closed, bounded three-file directory:

```text
manifest.json
model.onnx
NOTICE
```

The canonical manifest binds the exact upstream source and checkpoint
identities, converter and independent evidence identities, conversion
environment, graph contract, channel order, and member hashes. Runtime open
authenticates all three members, rejects links and unexpected files, loads the
graph from bytes read through the held model descriptor, validates graph
metadata, and executes a minimum-length probe before returning a kernel.

Correctness is established independently of conversion. A locked PyTorch
evidence helper authenticates and executes each original checkpoint separately
and records every state tensor plus exact selected-channel `f32` bits. Rust
qualification compares every resulting scalar with the converted graph using
absolute tolerance `1e-5`. Normal gates instead execute a tiny checked
same-schema ONNX fixture, so they need no Python, checkpoints, production model,
or network.

## Context

The upstream checkpoints are PyTorch containers. Loading them directly in the
service would retain Python/PyTorch as runtime dependencies and would not
provide a safe, fixed Rust artifact contract. A native Rust translation before
measuring an optimized CPU runtime would duplicate mature tensor operators and
create a larger compatibility surface.

One combined graph avoids twelve runtime sessions and keeps checkpoint order
observable. Retaining all twelve raw channels also avoids hiding ensemble and
post-processing behavior inside the conversion step. The checked
`pangopup-compat-v1` corpus already fixes those later semantics, but variant
construction and post-processing are a separate outcome.

## Consequences

- The raw CPU model kernel is shipped and numerically qualified; variant-level
  fallback, lookup-miss routing, model/reference/mask delivery, and HTTP remain
  future work.
- The production model is a separately built GPL asset and is not committed to
  Git. The checked miniature is the only repository ONNX file.
- The first runtime is CPU-only and single-owner. There is no accelerator,
  quantization, configurable-thread, pooling, `Sync`, or concurrency claim.
- Model artifact identity does not include the Rust compiler, ONNX Runtime,
  CPU, or thread policy. Those belong to qualification evidence, allowing a
  runtime update to be tested without falsely renaming unchanged model bytes.
- ONNX Runtime dependency provisioning may download a checksum-pinned native
  archive during a clean Cargo build. Once provisioned, model execution and
  normal tests perform no network request.
- Measurements show why precomputed SNV lookup remains authoritative: raw model
  calls are measured in seconds on the qualification host, while indexed
  lookup is measured in microseconds or less.
- MPS, CUDA, another runtime, thread tuning, or quantization may be considered
  only after replaying the same independent oracle and retaining a measured
  end-to-end benefit.
