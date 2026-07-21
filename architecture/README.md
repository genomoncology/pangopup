# Pangopup Architecture

Pangopup is an offline-built, mmap-served annotation engine for exact Pangolin
scores. The first runtime answers GRCh38 SNV queries from the Wagner/Neverov
precomputed dataset. Model inference and HTTP are later adapters.

## Boundaries

- [`design.md`](design.md) — typed API, crate ownership, lookup flow, and scope.
- [`index.md`](index.md) — candidate index shape, build invariants, validation,
  and performance method.
- [`source-data.md`](source-data.md) — dataset identity, observed properties,
  reference evidence, and CC BY obligations.
- [`decisions/`](decisions/) — accepted cross-cutting decisions.

Current work, unresolved priorities, and hypotheses belong in
[`../planning/`](../planning/). Observable CLI behavior belongs in
[`../spec/`](../spec/).
