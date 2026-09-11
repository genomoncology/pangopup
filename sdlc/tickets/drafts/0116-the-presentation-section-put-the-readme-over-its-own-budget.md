# The presentation section put the README over its own budget

`spec/readme-first-use.md` holds the root README to 1,770 words and 270 lines,
pins the exact list of `## ` headings, and pins the count of `![` image marks at
one. The README now stands at 1,829 words and 271 lines. It carries a
`## Watch a presentation on it` heading that the pinned list does not contain,
and a thumbnail that is a second `![` mark. Four assertions in that file are red
against the file they describe.

The budget is the rule that keeps the README a first-use guide, and it was
raised once on purpose and recorded. A section that spends fifty-nine words past
it without the rule moving leaves the rule saying one thing and the file doing
another, and the next change to the README meets a gate that was already red
before it arrived.

Done, observably:

- `make spec` passes the blocks in `spec/readme-first-use.md`.
- Either the README comes back inside 1,770 words and 270 lines, or the budget
  moves in the same commit with the reason recorded beside the number the way
  the previous move was recorded.
- The pinned heading list and the pinned image-mark count describe the file
  that ships.

Boundary: this changes no product source, alters no field, and adds no route,
flag or output. It settles the README budget and the two pinned lists, and it
does not revisit what any other README rule requires.
