---
---
# The transversion-only claim is held word for word and counted nowhere

`architecture/compatibility.md` and `spec/score-value.md` both publish "Every
variant in the measured set is a transversion and none of them is a
transition, so these figures cover transversions only".
`tests/route-disagreement-rate.sh` holds that sentence to the
`substitution-coverage` field of
`planning/artifacts/0059-route-disagreement-rate.md`, and requires the field to
name both substitution classes. Nothing counts the classes.

The set itself is committed. `planning/artifacts/0059-route-disagreement-set.tsv`
carries 2,615 variants, one per line, each written `GRCh38:<contig>:<pos>:<ref>:<alt>`.
Counting them is text arithmetic over a committed file: A<->G and C<->T are
transitions, every other unequal pair is a transversion. Counted on 2026-09-11
the file holds 2,615 transversions and 0 transitions, in four shapes: A->C 760,
C->G 541, G->T 532, T->A 782. So the published sentence is true today and the
gate would not notice if a later set made it false.

The gap matters because the set is drawn by a rule written down in the file's
own header, and a rule change is exactly how the sentence goes stale. The
`## Method` rule cycles A->C->G->T->A, and every step of that cycle crosses the
purine/pyrimidine boundary. A rule that cycled A->G would make every published
figure cover a class the sentence denies it holds, with both documents still
green.

A check belongs beside the ones `tests/route-disagreement-rate.sh` already
runs: read the set file, count the two classes, and refuse a coverage sentence
the count does not bear out. It costs no build and no model, the way the rest
of that file does not.

Ticket 0084 corrected and pinned the sentence. Counting the set was outside
what it asked for.
