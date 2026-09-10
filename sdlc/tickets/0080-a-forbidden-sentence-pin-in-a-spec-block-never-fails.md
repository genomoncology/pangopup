---
---
# A forbidden-sentence pin in a spec block never fails

`spec/*.md` blocks pin documentation both ways. A required sentence is pinned
as `printf '%s' "$doc" | rg -F -- 'sentence' >/dev/null`. A forbidden sentence
is pinned as the same pipeline with a leading `!`. The required form works. The
forbidden form does nothing at all.

`set -e` is defined to ignore a pipeline that begins with the `!` reserved
word, and a mustmatch bash block reports the status of its last command. Every
`! ... | rg ...` line in these blocks sits in the middle of a block, so a
forbidden sentence that reappears changes no exit status and no gate notices.

Measured in this checkout on 2026-09-10 while running the design stage of
ticket 0054. `spec/http-service.md:181` forbids the sentence `Store this
identity as the data-set version` in `README.md`. Appending exactly that
sentence to `README.md` and running `mustmatch test spec/http-service.md` left
the `What a consumer pins` block passing. Breaking a required sentence in the
same block failed it, so the block runs and the required pins bite. Only the
forbidden pins are inert.

`rg -n '^! ' spec/` reports the population. Each one is a claim someone thought
was gated.

Done, observably:

- A forbidden sentence that reappears in a pinned document fails `make spec`.
- Every existing forbidden pin is either converted or removed with a reason.
- The conversion is mechanical enough that a new one cannot be written the
  inert way without something saying so.

Boundary: this changes how the spec blocks assert. It states no new claim about
what any document must say, and it must not weaken a required pin to make a
forbidden one work.
