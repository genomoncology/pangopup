# Pangopup

Pangopup is a standalone GPL-3.0 Rust project for fast Pangolin-compatible
splice scoring. Its first product slice turns the published GRCh38 SNV dataset
into a compact immutable index, memory-maps that index, and exposes exact lookup
through a small CLI. Later slices add bundled model fallback and an HTTP service
without depending on another application or service.

The lookup engine is deliberately separate from the on-disk format. Callers use
typed Rust capabilities; only the index crate knows byte layouts, offsets, and
memory mapping. A model-backed path for variants absent from the SNV dataset and
an HTTP adapter are later work, not part of the initial contract.

## Current state

Implemented today:

- the four-crate Rust workspace and strict lint/test/spec gates;
- CLI help/version behavior with two executable smoke specs;
- GPL-3.0 source licensing and CC BY 4.0 dataset attribution;
- a retained Rust analyzer that scanned the complete downloaded score corpus;
- complete-corpus entropy, sparsity, and candidate-format measurements;
- the standalone API, runtime-data, delivery, and performance decisions.

Not implemented yet: public score types or traits, source fixture and validator,
index writer, mmap reader, real CLI lookup, artifact installer, model runtime,
HTTP service, or result cache.

Start with:

- [`architecture/README.md`](architecture/README.md) for the technical design;
- [`planning/frontier.md`](planning/frontier.md) for the current work boundary;
- [`NOTICE`](NOTICE) for the precomputed dataset attribution.

## Workspace

- `pangopup-core` — public typed vocabulary and provider capabilities;
- `pangopup-index` — private format codec and validated mmap reader;
- `pangopup-build` — offline source validator and streaming index builder;
- `pangopup-cli` — observable command-line adapter, installed as `pangopup`.

## Development

```bash skip
make lint
make test
make spec
```

The source code is licensed under GPL-3.0-only. The source Pangolin precomputed
scores remain a separately attributed CC BY 4.0 dataset.
