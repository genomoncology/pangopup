---
---
# A legal-advice denial is satisfied by an unrelated negation

`tests/repository-sourcing.sh` requires every sentence in
`architecture/motivation.md` that names legal advice to deny it. It reads the
claim span, which runs from the start of the sentence through the end of
"legal advice", and passes the sentence when any negation stands anywhere in
that run. A negation that belongs to a different clause counts.

So "This repository states no conclusion about any other program's licence, and
this page is legal advice" passes: the `no` in the first clause answers for the
second. The sentence the gate exists to refuse is exactly this one, written as
a trailing clause on a sentence that already carries a negation. Measured on
this tree during review, before the denial was made a sentence of its own.

The span is the right idea, since a negation standing after the term is about
something else. What it needs is a narrower start: the clause the term stands
in rather than the whole sentence.
