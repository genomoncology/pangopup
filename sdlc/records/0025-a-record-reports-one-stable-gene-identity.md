---
base: 53b022a
head: 2dd0099930351529c47b2338ba689926993b62d1
---

# A record reports one stable gene identity

Every structured score record now reports `stable_gene` beside the unchanged source `gene`. Precomputed records repeat their stable Ensembl identifier. Model records use the stable component of the versioned GENCODE identity, including `_PAR_Y` forms.

The new field supports grouping and filtering across lookup and model routes. Consumers must retain `gene` wherever exact version or PAR identity matters. Human-readable output, core score types, routing, cache data, provenance, scores, asset identities, and gene-filter behavior remain unchanged.

Design review corrected the original ticket before implementation. It named the public field and limited its semantics to the stable grouping and filter component. Independent code review mechanically verified every changed fixture and accepted the implementation without findings. PangoPup CLI tests, 74 relevant specifications, production release qualification, Clippy, formatting, and `git diff --check` passed.
