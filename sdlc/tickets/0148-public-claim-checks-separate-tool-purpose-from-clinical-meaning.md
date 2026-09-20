---
flow: build
priority: 1
deps: []
---
# Public claim checks separate tool purpose from clinical meaning

## Outcome

PangoPup may plainly say that its scores predict possible splice changes. Public text must not claim that a score alone proves pathogenicity, clinical significance, or a diagnosis. The checker applies that rule to the README and motivation page and verifies a legal-advice denial in its own clause.

## Decision

The sentence “The scores help identify variants that may alter RNA splicing” describes the model target and remains allowed. It does not assign clinical evidence strength. The narrower clinical-interpretation boundary replaces the overbroad rule that reader-facing material cannot say what a score is evidence for.

## Done, observably

- Update the repository motivation specification to state the purpose-versus-clinical-meaning boundary in direct language.
- Check both `README.md` and `architecture/motivation.md` for forbidden clinical interpretation claims. Fixtures distinguish the accepted splice-target sentence from claims of pathogenicity, clinical significance, diagnosis, or stand-alone evidence.
- Require a denial of legal advice within the same clause as the term in both public pages. Preserve one accepted denial fixture from each page.
- Check every occurrence of `legal advice`. An unrelated negation in an earlier clause must not satisfy it, and a denied first occurrence must not hide a later affirmative occurrence.
- Keep the lexical rule bounded, documented, and supported by red mutation fixtures. Do not claim general prose understanding.
- Archive drafts 0118 and 0120. Update the durable record and frontier. `make lint`, `make test`, and `make spec` pass.

## Boundary

Change public wording only when needed to express this decision, plus repository checks, fixtures, and specifications. Do not change score semantics, clinical policy, model output, or public APIs.
