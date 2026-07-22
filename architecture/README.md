# Pangopup Architecture

Pangopup's target combines exact published Pangolin SNV lookup with compatible
model inference. The shipped functional runtime answers GRCh38 SNV queries from
the Wagner/Neverov precomputed dataset through a fixed 11-byte mmap index and
typed CLI. Model fallback, asset installation, and HTTP remain future work on
the same standalone Rust core.

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
- [`service.md`](service.md) — planned lookup-first HTTP boundary, foreground
  lifecycle, deployment, and operational proof.
- [`decisions/`](decisions/) — accepted cross-cutting decisions.

Current work, unresolved priorities, and hypotheses belong in
[`../planning/`](../planning/). Observable CLI behavior belongs in
[`../spec/`](../spec/).
