---
flow: build
priority: 5
---
# A deterministic streaming writer produces sparse SNV candidate bytes

ADR 0027 requires a complete smaller candidate, but only fixed-v1 has a production writer. The historical sparse writer was benchmark-only and held the corpus in memory. Define the candidate's bytes and build them without retaining score payload proportional to the corpus. Reader trust and lookup remain a separate ticket.

Done, observably:

- A candidate-only writer accepts the same complete-gene `InputLocus` stream as fixed-v1 and emits a distinct versioned sparse-direct file. It never changes `StreamingIndexWriter`, `BundleOpen`, manifests, profiles, or active routing.
- Two builds from the same ordered input are byte-identical. A small independent test decoder round-trips every field without using future reader code.
- Miniatures cover overlaps, coordinate gaps, all reference bases, default records, nondefault gain and loss pairs, zero scores with nondefault positions, rank and block boundaries, both real `REF=N` omitted-base shapes, and maximum coordinates.
- The file declares exact length, bounded section ranges and counts, ordered gene/segment and block directories, fixed reserved bytes, and the exact candidate format identity. Every length, offset, count, multiplication, addition, and conversion uses checked arithmetic.
- Writer memory evidence spans creation, every gene submission, and `finish`, including final file assembly. Peak and retained heap and Linux resident-memory measurements prove the writer never materializes or retains score payload proportional to output. The check fails if final assembly collects the artifact in memory.
- The writer refuses an empty gene, mixed genes, repeated or decreasing gene submissions, coordinates not strictly increasing by contig then position, any ordinary/ambiguous duplicate at one gene-locus, malformed ordinary alternate sets, invalid ambiguous alternate sets, and gain/loss relative positions outside -50…50. A failed submission poisons the writer; `finish` cannot publish the previously accepted prefix.
- Existing output and scratch paths are never overwritten or removed. Injected spooling and final-assembly I/O failures leave no completed output, preserve every pre-existing path byte-for-byte, and remove only writer-owned temporary files. Success also leaves no writer-owned scratch file behind.
- `architecture/index.md` records the candidate format identity, deterministic streaming construction, byte layout, and candidate-only status. The completion record names the tests; neither document claims reader safety or any ADR 0027 full-corpus gate.

Boundary: do not add a public reader or lookup, generate the complete genome candidate, benchmark, alter bundle/runtime identities, route production through the candidate, publish assets, or claim size, latency, parity, or corruption qualification. Historical code is algorithm evidence only.
