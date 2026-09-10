---
flow: build
priority: 2
---
# A published index size is the measured one, not a computed one

`architecture/index.md:75` states that the complete corpus is 15,030,604,105
bytes. The retained build artifact records the payload as 15,030,603,775 bytes
at `planning/artifacts/003-full-index-build.md:58`. The difference is 330 bytes,
which is exactly thirty loci at eleven bytes each: the published figure is
1,366,418,555 loci multiplied by eleven, and thirty of those loci are held in a
separate exception section rather than in the fixed-width payload. The document
presents an arithmetic product as a measurement.

The number is small and the error is small, and both are in a public document
that a reader may check against a file they downloaded. A reader who measures
the payload and finds a different size than the architecture document states has
no way to tell which is wrong.

Done, observably:

- Every byte size published in the architecture folder is one that was measured,
  and a reader who measures the same artifact gets the same number.
- Where a figure is derived by arithmetic rather than measured, the document
  says so and says what it is derived from.
- The loci held outside the fixed-width payload are accounted for where the
  payload size is stated, so the two figures reconcile on the page rather than
  in a reader's head.
- A gate compares each published size against the retained artifact that
  measured it, so a figure drifting from its source turns `make spec` red.
- `make test`, `make spec` and `make lint` pass.

Boundary: this ticket changes no product source under `crates/`, rebuilds no
index, re-measures nothing, and changes no on-disk format, layout or file. It
corrects what is published about artifacts that already exist and are already
certified. The certified installed member size of 15,033,158,255 bytes is
correct wherever it appears and is not touched. It does not revisit what ticket
0056 settles, and it names no private consumer of this software.
