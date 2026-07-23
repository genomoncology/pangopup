# Pangopup Architecture

Pangopup's target combines exact published Pangolin SNV lookup with compatible
model inference. The shipped functional runtime answers GRCh38 SNV queries from
the Wagner/Neverov precomputed dataset through a fixed 11-byte mmap index and
typed CLI, plus Linux local installation, active-bundle discovery, the
immutable public `snv-grch38-v1` release, and pinned resumable remote sync.
Model fallback and HTTP remain future work on the same standalone Rust core.

## Boundaries

- [`design.md`](design.md) — typed API, crate ownership, lookup flow, and scope.
- [`index.md`](index.md) — candidate index shape, build invariants, validation,
  and performance method.
- [`source-data.md`](source-data.md) — dataset identity, observed properties,
  reference evidence, and CC BY obligations.
- [`runtime-data.md`](runtime-data.md) — the exact local assets needed for
  standalone lookup and model fallback.
- [`delivery.md`](delivery.md) — release assets, installation, and immutable
  bundles.
- [`decisions/0007-deterministic-snv-transport.md`](decisions/0007-deterministic-snv-transport.md)
  — accepted no-tar transport, deterministic codec boundary, and verification
  layers.
- [`service.md`](service.md) — planned lookup-first HTTP boundary, foreground
  lifecycle, deployment, and operational proof.
- [`decisions/`](decisions/) — accepted cross-cutting decisions.

Current work, unresolved priorities, and hypotheses belong in
[`../planning/`](../planning/). Observable CLI behavior belongs in
[`../spec/`](../spec/).
