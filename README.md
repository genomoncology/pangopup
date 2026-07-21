# Pangopup

Pangopup is a GPL-3.0 Rust workspace for fast, exact lookup of precomputed
Pangolin splice scores. Its first product slice turns the published GRCh38 SNV
dataset into a compact immutable index, memory-maps that index, and exposes the
lookup through a small CLI.

The lookup engine is deliberately separate from the on-disk format. Callers use
typed Rust capabilities; only the index crate knows byte layouts, offsets, and
memory mapping. A model-backed path for variants absent from the SNV dataset and
an HTTP adapter are later work, not part of the initial contract.

## Current state

The repository is at architecture and walking-skeleton stage. The CLI currently
provides only identity and help output. No score lookup is implemented yet.

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
