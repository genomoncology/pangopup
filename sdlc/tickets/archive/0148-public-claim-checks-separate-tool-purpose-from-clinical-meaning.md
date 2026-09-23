---
flow: build
priority: 1
deps: []
---
# Public claim checks separate tool purpose from clinical meaning

## Outcome

PangoPup may plainly say that its scores predict possible splice changes. Public text must not claim that a score alone proves pathogenicity, clinical significance, or a diagnosis. The checker applies that one rule to the README and motivation page.

## Decision

The sentence “The scores help identify variants that may alter RNA splicing” describes the model target and remains allowed. It does not assign clinical evidence strength. The narrower clinical-interpretation boundary replaces the overbroad rule that reader-facing material cannot say what a score is evidence for.

## Done, observably

- Update the repository motivation specification to state the purpose-versus-clinical-meaning boundary in direct language.
- Check both `README.md` and `architecture/motivation.md` for forbidden clinical interpretation claims. Fixtures distinguish the accepted splice-target sentence from claims of pathogenicity, clinical significance, diagnosis, or stand-alone evidence.
- In both public pages, accept “A score alone does not establish a diagnosis” and reject its affirmative counterpart. The checker must distinguish a stated limitation from a forbidden claim instead of banning clinical words wherever they appear.
- Keep the lexical rule limited to those four clinical meanings and the two named pages. Support it with direct positive and negative fixtures; do not claim general prose understanding.
- Archive draft 0120. Leave the unrelated legal-advice draft 0118 untouched. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change the two named pages only if needed to express this decision, plus the one existing claim checker and its focused fixtures. Do not alter legal-advice checks, other pages, score semantics, clinical policy, model output, or public APIs.
