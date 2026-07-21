# 0001 — Typed provider over an opaque immutable index

Status: accepted
Date: 2026-07-20

## Decision

Callers consume typed Pangolin score records through a narrow Rust provider
capability. Binary sections, packed fields, mmap lifetimes, source-file parsing,
and index paths remain private to `pangopup-index`.

One long-lived reader opens one immutable, source-identified bundle for its
process lifetime. Index replacement requires a new process.

## Consequences

- The storage format can evolve without changing CLI or future HTTP consumers.
- Mmap safety and corrupt-file validation have one owner.
- The OS page cache is the baseline; extra caches require measurement.
- Results are small owned values rather than public borrows into mapped bytes.
- Builders own source validation, deterministic output, provenance, and
  attribution.
