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

An entry is keyed by the literal GRCh38 variant. The digest accelerates lookup,
but a hit requires equality with the complete canonical key. Values are
canonical JSON decoded through the normal typed score constructors.

The identities are recorded once for the file, as readable text a reader can
check: the running software version, scoring semantics, model
bundle/profile/graph, reference bundle/profile/sequence set, mask length/hash,
masking policy, and window. The file is judged once, when it opens, and
discarded whole when any of them changed. No migration keeps old rows readable.
Ticket 0040 measured that no thread or worker setting moves any published field,
so the CPU policy is neither keyed nor recorded. A discard is reported on
standard error, naming the file, so an explicitly selected database is never
*silently* replaced.

## Why

Model inference takes seconds, while SQLite provides persistent reuse across
process restarts. SQLite is an optimization only: malformed rows become misses,
cache write failure cannot invalidate an already computed answer, and the
authoritative SNV mmap remains first.

Manifest admission and mask identification happen before a hit. A validated
hit does not open or hash the dense reference, load the ONNX graph, construct a
session, run its initialization probe, or perform inference. A miss
authenticates the full components and confirms that their identities still
match the recorded setup before scoring.

## Consequences

The cache uses bundled SQLite, WAL, a versioned application/schema identity,
private same-user filesystem permissions, deterministic insertion/update-order
eviction, read-only valid hits, and safe disposable-default recreation. It is
not provenance and is not shared between machines.
