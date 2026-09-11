---
---
# A `like` pin over a whole record absorbs every field added to it

`mustmatch EXPECTED` compares canonical JSON when both sides parse as JSON, so
an added field fails the pin. `mustmatch like EXPECTED` compares a subset, so
an added field passes. Measured against mustmatch 0.1.0 on 2026-09-11:
`printf '{"a":1,"b":2}' | mustmatch '{"a":1}'` exits 1 and
`printf '{"a":1,"b":2}' | mustmatch like '{"a":1}'` exits 0.

`spec/` carries 24 JSON-object pins written with `like`. Some are deliberately
partial -- `spec/full-bundle.md:32` pins three fields of an eleven-field build
summary because the paragraph above it is about determinism, not the summary.
Others present a whole record, including a complete `provenance` object, and a
reader takes them as the bytes the command prints.

Measured on 2026-09-11 by rewriting every `| mustmatch like '{` in `spec/` to
`| mustmatch '{` and running `make spec`: three of the 24 fail.

- `spec/snv-lookup.md:17` and `spec/model-routing.md:100` both pin a whole
  lookup record whose `provenance` object does not carry
  `"software_version":"0.5.0"`. The product emits it. It arrived in `d37d558`
  on 2026-09-09 and neither transcript changed.
- `spec/full-bundle.md:32` is the deliberate partial pin described above.

This is the staleness ticket 0085 found in `spec/model-kernel.md`, where the
inspect transcript had been missing `"representation":"singleton"` since
`c815901` on 2026-07-26. There the fence pinned nothing at all. Here the fence
pins, and still lets a field arrive unnoticed, so the gate stays green and the
published transcript stops being what the command prints.

Whoever takes this has to decide per pin whether the transcript is a whole
record or a deliberate excerpt, tighten the whole-record ones to the exact
form, and leave a way to tell the two apart -- a `like` on a whole record
cannot be distinguished from a `like` on an excerpt by reading the line.

Found during the code review of ticket 0085.
