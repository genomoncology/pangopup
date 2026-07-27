# ADR 0019: Persist complete model results in SQLite

## Decision

Pangopup stores successful, complete, unfiltered model results in one
disposable SQLite database. Precomputed SNV hits, rejected variants, partial
answers, operational failures, and rendered CLI bytes are never cached.

The default database is
`${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/model-results.sqlite3`, with a
10,000-entry insertion/update-order bound. Operators may select another
positive bound or `unlimited`. An explicitly selected database is never
silently replaced.

Each entry is keyed by the literal GRCh38 variant and every identity or policy
that can change its score: scoring semantics, model bundle/profile/graph, CPU
policy, reference bundle/profile/sequence set, mask length/hash, masking policy,
and window. The digest accelerates lookup, but a hit requires equality with the
complete canonical key. Values are canonical JSON decoded through the normal
typed score constructors.

## Why

Model inference takes seconds, while SQLite provides persistent reuse across
process restarts. SQLite is an optimization only: malformed rows become misses,
cache write failure cannot invalidate an already computed answer, and the
authoritative SNV mmap remains first.

Manifest admission and mask identification happen before a hit. A validated
hit does not open or hash the dense reference, load the ONNX graph, construct a
session, run its initialization probe, or perform inference. A miss
authenticates the full components and confirms that their identities still
match the admitted key before scoring.

## Consequences

The cache uses bundled SQLite, WAL, a versioned application/schema identity,
private same-user filesystem permissions, deterministic insertion/update-order
eviction, read-only valid hits, and safe disposable-default recreation. It is
not provenance and is not shared between machines.
