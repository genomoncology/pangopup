---
---
# A published entropy size does not follow from the figure printed above it

`architecture/index.md` prints two numbers as one derivation. Lines 60 to 64
say the correlated model "lowers the result to:" and then show

```text
5.995913 bits per locus
1,024,115,911 bytes total
0.954 GiB total
```

A reader who multiplies the first line by the locus count the same document
publishes gets a different second line. 5.995913 bits times 1,366,418,555 loci,
divided by eight, is 1,024,115,847 bytes. The published figure is 64 bytes
larger. The gap is consistent with the bytes figure having been computed from
full-precision bits and the bits figure having been rounded for display, but
the document does not say so, and the two lines stand together as though one
follows from the other.

Line 57 has the same shape one paragraph earlier. "Modeling the reference and
three alternate score records as separate symbol streams gives 1,285,518,889
bytes" does not follow from the 1.848462 bits on line 56 either: that product
is 1,262,886,388 bytes. The figure is presumably the sum of four separate
per-stream entropies, and nothing in the section says that.

This is the family ticket 0084 is about, found outside the section 0084
repairs. 0084 corrected `## Selected fixed-width v1`, where a product was
presented as a measurement, and its gate covers that one section. Nothing
reads `## Entropy result`.

Done, observably:

- A reader can reproduce every figure in `## Entropy result` from figures the
  section publishes, or the section says what the figure was computed from and
  at what precision.
- `make lint`, `make test` and `make spec` pass.

Boundary: documentation only. It changes no product source, re-measures
nothing, and publishes no new number: the entropy results stand as measured
and only what the document says about how they were reached changes.
