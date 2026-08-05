# 057 — Add citation metadata and prior-art links

Status: ready

## Why

PangoPup v0.3.0 is published, but the repository does not provide standard
machine-readable citation metadata. The README attributes Pangolin and the
Zenodo-derived lookup data, but it does not give readers one clear prior-art
section linking the software, paper, and source dataset. The previously
accepted benchmark scope is deferred at Ian's request; this ticket keeps only
the useful citation work.

## Scope

- Add a root `CITATION.cff` using CFF 1.2.0 for PangoPup v0.3.0, naming Ian
  Maurer as the software author, linking the public repository and immutable
  v0.3.0 release, and recording the 2026-08-05 release date.
- Add a compact prior-art section near the bottom of `README.md` with direct
  links and unambiguous attribution for:
  - Tony Zeng's GPL-3.0 Pangolin software repository;
  - Tony Zeng and Yang I. Li's 2022 Genome Biology Pangolin paper and DOI; and
  - Nils Wagner and Aleksandr Neverov's CC BY 4.0 Pangolin precomputed-score
    dataset and DOI.
- Add an offline repository test that parses the citation file and checks the
  stable identity, release, author, repository, and license fields. Do not add
  a runtime dependency.
- Keep existing license/NOTICE attribution intact. Do not claim that PangoPup
  authored Pangolin, the trained model, or the Zenodo dataset.
- Do not add benchmarks, performance claims, generated measurement artifacts,
  or download/rebuild any large asset. Benchmarking remains deferred.

## Success Checklist

- `CITATION.cff` is valid CFF 1.2.0 and GitHub can recognize it as repository
  citation metadata.
- It names Ian Maurer without inventing an ORCID and identifies immutable
  release v0.3.0, release date 2026-08-05, repository URL, release URL, and
  GPL-3.0-only license.
- The README directly links the Pangolin GitHub repository, the paper at DOI
  `10.1186/s13059-022-02664-4`, and Zenodo DOI
  `10.5281/zenodo.15649338`, with the correct creators and licenses.
- An offline test rejects missing or drifted required citation fields.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Separate citation from benchmarking.** The benchmark harness requires a
   quiet host and is not required for product correctness. Citation metadata
   and prior-art links are independently useful and can ship without measured
   performance claims.
2. **Describe PangoPup, cite dependencies separately.** `CITATION.cff` names
   Ian Maurer as PangoPup's author. The README, LICENSE, and NOTICE retain
   explicit upstream/data attribution so authorship is not conflated.
3. **Pin the existing public release.** Citation metadata describes v0.3.0,
   whose public release is already qualified. This ticket does not publish a
   new release or alter runtime behavior.

## Dependencies

Public PangoPup v0.3.0 and its existing LICENSE/NOTICE attribution.

## Notes

- PangoPup repository: `https://github.com/genomoncology/pangopup`
- PangoPup release: `https://github.com/genomoncology/pangopup/releases/tag/v0.3.0`
- Pangolin software: `https://github.com/tkzeng/Pangolin`, Tony Zeng,
  GPL-3.0.
- Pangolin paper: Tony Zeng and Yang I. Li, “Predicting RNA splicing from DNA
  sequence using Pangolin,” Genome Biology 23, 103 (2022), DOI
  `10.1186/s13059-022-02664-4`.
- Dataset: “Pangolin precomputed scores,” Nils Wagner and Aleksandr Neverov,
  published 2025-06-12, CC BY 4.0, DOI `10.5281/zenodo.15649338`. Zenodo does
  not declare a dataset version; do not call it v1.
- Do not add an ORCID, email address, affiliation, or other author fact that
  is not already authenticated in the repository.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05.

The earlier benchmark-and-citation Ticket 057 was accepted but not implemented.
Ian explicitly deferred benchmarking on 2026-08-05. This materially narrower
ticket therefore returns to independent design review before implementation.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket057_citation_design_review`, 2026-08-05.

Verdict: ACCEPT. The reviewer authenticated public v0.3.0 at commit
`3a857f7def2c11ad9d9e38ed62b7204bf7d6b691`, the release date, workspace
version/license, Ian Maurer's repository identity, and all three prior-art
attributions. Implementation must validate required CFF fields with a real
YAML parser in test scope, record successful schema validation, and make the
paper URL directly clickable. No material design finding remains.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
