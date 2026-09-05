---
base: 3b733c65336294a9dbc17f557fde22cb7156c3b7
head: 55559d57345c748259525f1950264d2e0bd06ec2
---

# Report the active scoring contract

The HTTP status route now reports one machine-readable `request_contract`. The object covers the scoring API version, route, content type, body limit, batch limits, uncached model-work limit, assembly, allele limits, all accepted variant forms, all primary-contig spellings, gene-filter forms, and the optional `model_only` field. Existing status fields remain available.

Design review first rejected the ticket because it did not define the schema or require reported limits and parser vocabulary to share their enforcement sources. The amended design fixed the exact additive schema. One `RequestLimits` value now drives service enforcement and reporting. Exported engine constants drive validation and reporting. One contig-spelling helper drives parsing and serialization. The cost is a larger explicit public contract that future request changes must maintain.

Implementation review found no remaining defect. An allocation regression appeared during implementation because the contig helper initially built owned values during parsing. The implementer replaced it with static borrowed spellings before independent code review. The parser keeps its native fast path and allocates no descriptor list per candidate. `pangopup-core` remains unchanged because its source contributes to immutable asset fingerprint evidence.

Root verification passed `git diff --check`, `make lint`, `make test`, and `make spec`. The executable specification reported 282 passed and 7 skipped. Boundary tests derive each numeric limit from serialized status and exercise the limit and one value beyond it. Tests also pass every reported contig spelling through the production parser and cover each reported variant and gene-filter form. The README remains inside its existing compactness limit at 258 lines and 1,695 words. Existing dependency-duplication warnings remained non-failing. The shared hardening issue remains because it contains held observations beyond this ticket.
