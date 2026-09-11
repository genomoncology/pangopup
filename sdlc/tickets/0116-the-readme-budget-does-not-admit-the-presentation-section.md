---
flow: build
priority: 1
---
# The README budget does not admit the presentation section

`spec/readme-first-use.md` holds the README to a first-use guide by pinning its
size, its headings and its image marks. The README gained a presentation section
in commit `e14e822` and four of those assertions now fail on `main`. Measured in
this checkout on 2026-09-11:

- `wc -w < README.md` is 1,829 against `test … -le 1770`.
- `wc -l < README.md` is 271 against `test … -le 270`.
- The pinned heading list does not carry `## Watch a presentation on it`.
- The pinned image count is 1 and the file now holds 2.

`make spec` is red on `main` for that reason alone. Every other gate passes.

The section is deliberate published content: a heading, three sentences of
description, a thumbnail linking to the talk, and two links to the recording and
its write-up. It says why the project exists, which is the thing a reader
arriving at this repository has never been told.

Settled: the section stays as written. The budget moves to admit it. A budget
that refuses the maintainer's own published content is measuring the wrong
thing, and cutting the guide to pay for a section that explains the project
trades the better text for the worse.

Settled: the new number is the size of the file with this section in it, not a
round number above it. A ceiling set above what stands leaves room nobody
argued for.

Settled: this ticket does not revisit the rule that a later addition is paid for
out of the guide rather than added to it. That rule governs the next sentence,
and the paragraph in `spec/readme-first-use.md` that records it stays.

Done, observably:

- `make spec` passes on a tree carrying the presentation section, and the
  README keeps every word, link and image that section added.
- The pinned heading list and the image count describe the file as it stands,
  and each still refuses a heading or an image the file does not carry.
- The size assertions still refuse growth: a sentence added to the README
  beyond the new figures turns `make spec` red, proved by adding one.
- The file records what the new numbers are, what they were, and what moved
  them, so a later reader finds the reason beside the number.
- `make lint`, `make test` and `make spec` pass.

Boundary: this ticket changes no product source under `crates/`, alters no
score, position, status, reason or provenance field, and adds no route, flag or
output. It does not edit the presentation section, its wording, its image or its
links. It does not touch `architecture/motivation.md` or the link to it, which
is ticket 0077. It publishes no new measurement and names no private consumer of
this software.
