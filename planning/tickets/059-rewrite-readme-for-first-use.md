# 059 — Rewrite the README for first-time users

Status: ready

## Why

The README is technically accurate but reads like an internal qualification
record. It repeats disclaimers and attribution, leads with headings and release
history instead of the product, and exposes implementation/security rationale
that a new user does not need. A fresh independent read found that the useful
content can be presented in roughly half the current document while improving
input guidance and first-use success.

## Scope

- Rewrite `README.md` as a compact user guide with this order:
  1. title followed immediately by two short paragraphs explaining what
     PangoPup is, what it predicts, and its lookup-first/model-fallback value;
  2. `Quick start` for the direct Linux CLI;
  3. `Input and output` semantics and common CLI options;
  4. `HTTP service`;
  5. `Docker`;
  6. compact `Storage and operations`; and
  7. `Citation and license`.
- Do not add `Introduction` or `What it predicts` headings. Do not lead with a
  list of absent features. Explain the product positively and directly.
- Remove user-irrelevant material from the root README:
  - v0.2.0 history and repeated “immutable” release-engineering language;
  - HGVS/transcript/gene-knowledge and similar non-feature inventories;
  - exact-byte storage defenses and mmap implementation rationale beyond one
    useful sentence that the index is mapped rather than loaded wholly into
    RAM;
  - single-host benchmark methodology, PSS/RSS definitions, raw-evidence and
    `planning/` links;
  - source-build qualification commands and commit-recording instructions;
  - cache-path precedence internals;
  - digest-pinning deployment guidance;
  - the Apple `cpuinfo` warning investigation/history;
  - security implementation assurances for uninstall internals;
  - duplicated license/attribution defenses; and
  - maintainer links, build CLI, and repository gate instructions.
- Keep the exact user facts needed for successful use:
  - local GRCh38 Pangolin-compatible scoring;
  - precomputed SNV lookup and CPU model fallback for lookup misses and
    supported non-SNVs, with persistent reuse of model results;
  - direct binary Linux x86-64/GLIBC 2.39 requirement and native AMD64/ARM64
    Docker availability;
  - about 2.44 GiB download, 14.76 GiB installed, and 25 GB free for sync;
  - explicit sync and network-free scoring afterward;
  - variant grammar `GRCh38:CONTIG:POS:REF:ALT`, 1-based genomic coordinates,
    accepted contig forms, uppercase literal A/C/G/T alleles, no automatic
    trimming/alignment/normalization, anchored indel form, reference checking
    on model scoring, and model allele-length boundary;
  - JSON Lines default, table option, batch, gene filter, `--model-only`,
    result-status meanings, multiple gene records, gain/signed-loss and
    genomic-offset semantics, and `provenance.kind`. Explain compactly that
    `not_found` means no score record was produced—not a predicted zero effect—
    and cover the precomputed ambiguity statuses without exposing internal
    implementation history;
  - foreground HTTP command, readiness and one scoring example, batch/model
    admission limits, 429 behavior, and loopback/authentication/TLS warning;
  - one coherent Docker volume/sync/service path, one Docker CLI lookup, volume
    persistence, CPU-only inference, and Apple Silicon ARM64 support;
  - terse XDG paths, offline reuse, truthful direct-binary and Docker update
    paths that preserve compatible assets/volumes, code-only/full uninstall,
    and a practical default-service memory starting point; and
  - one nonduplicated citation/license section linking `CITATION.cff`, Pangolin
    software and paper, Zenodo dataset, GPL-3.0-only, CC BY 4.0, GENCODE v38,
    and `NOTICE`.
- Keep the scientific explanation in prose near the title: PangoPup reports
  the strongest predicted splice-site gain and signed loss and their genomic
  offsets. Distance 50 means 50 bases on either side; a supported deletion's
  allele span can extend the positive offset. Do not describe these as
  tissue-specific or clinical/pathogenicity classifications.
- Replace `spec/readme-first-use.md` with section-bounded user-contract checks
  for the new outline. Tests must check required commands/facts within their
  owning sections and explicitly reject removed internal-history phrases and
  root README links into `planning/` or `AGENTS.md`.
- Update the existing `pangopup-build` documentation-catalog unit test so it
  no longer requires the public README to advertise maintainer-only
  `pangopup-build --help`. Keep the catalog's maintainer FAQ/frontier checks;
  do not change build-CLI behavior.
- Update the citation README-link test so the Pangolin paper may use either
  its DOI resolver or the authoritative direct Springer article URL supplied
  by the user. Keep creator and paper identity assertions unchanged; do not
  weaken Pangolin-repository or Zenodo-link checks.
- Preserve `CITATION.cff`, `NOTICE`, runtime behavior, assets, releases, and
  detailed engineering evidence elsewhere. This ticket changes user-facing
  documentation and its static specification only.

## Success Checklist

- The title is followed directly by a plain-English product description, with
  no `Introduction` or `What it predicts` heading and no general “does not”
  inventory.
- A technically competent new user can install, sync, score an SNV, understand
  the result, and find model/HTTP/Docker usage in one linear read.
- The README is at most 220 lines and 1,700 words without packing prose into
  excessively long lines to satisfy the bound.
- Every retained command matches v0.3.0 CLI behavior, and every scientific,
  input, output, storage, service, and platform claim is exact.
- Removed internal/release-history material is absent rather than relocated to
  another root section.
- Section-bounded offline tests prove the outline, essential commands/facts,
  and absence of stale/internal material.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Rewrite rather than patch.** The problem is document hierarchy and
   repetition, not one paragraph. Preserve facts, not the existing prose.
2. **Positive product description.** Actionable constraints live beside the
   relevant input, platform, or service instructions; an opening catalog of
   unrelated missing features is removed.
3. **One user path, then modalities.** Direct CLI is the shortest first result.
   HTTP and Docker follow after readers understand the input/output contract.
4. **Evidence stays outside onboarding.** Keep a few rounded capacity and
   performance expectations useful to operators, but exclude methodology and
   internal artifact links from the README.

## Dependencies

Public PangoPup v0.3.0, Ticket 057 citation metadata, and the fresh-eyes review
by `/root/readme_fresh_eyes_review` on 2026-08-05.

## Notes

- Verify allele grammar and maximum length against the actual current parser
  and engine tests before writing; the fresh review identified this as a
  useful missing fact, not an already authenticated README claim.
- Prefer `pangopup <command> --help` over reproducing advanced path/cache
  precedence.
- The existing detailed engineering and release evidence remains in
  architecture/planning/history; do not copy or move it as part of this ticket.

## Coordinator Authorship

Coordinator: Codex (`/root`), 2026-08-05.

## Independent Ticket Review

Reviewer: Codex subagent `/root/ticket059_readme_design_review`, 2026-08-05.

Verdict: ACCEPT. The reviewer authenticated the CLI/parser/engine/HTTP facts,
confirmed that the proposed hierarchy and removal list directly address the
user's complaint, and found the compactness and section-bounded tests adequate.
Implementation should explain anchored indels in plain English, describe
distance 50 as the reported search region rather than the model's full context,
scope the 100-base limit to model scoring, and frame 256 MiB as a starting
allocation. No material design finding remains.

Code review later exposed three scope clarifications: status needed actual
meanings, pinned-v0.3.0 reinstall wording was not an update path, and the
separate platform section repeated facts already owned by Quick start, HTTP,
and Docker. Resolution: require compact status meanings, truthful selected-
release/Docker replacement guidance, and remove the redundant platform section
from the exact outline. This material outline correction returns to the same
ticket reviewer before implementation remediation.

Re-review verdict: ACCEPT. The reviewer confirmed the revised ownership of
platform facts, status semantics, and update guidance is coherent, user-facing,
and does not reintroduce internal history. No new material design issue remains.

## Implementation Evidence

Developer: Codex subagent `/root/ticket059_readme_implementation`, 2026-08-05.

- Replaced the 350-line internal-history-shaped README with a 220-line,
  1,137-word first-use guide. The title now flows directly into two product
  paragraphs, followed by one direct CLI quick start and progressively deeper
  CLI, HTTP, Docker, storage/operations, and citation guidance.
- Authenticated the retained user contract against the current parser, engine,
  renderer, HTTP admission code, installer command, and published container
  identity. In particular, the rewrite pins the literal allele grammar,
  anchored-indel shape, 100-base model boundary, signed genomic offsets,
  100-request/10-model-miss HTTP limits, and 429 queue behavior.
- Removed release history, absent-feature inventories, benchmark methodology,
  exact-byte defenses, source-build qualification, cache precedence internals,
  deployment digest advice, Apple warning history, maintainer commands, and
  root links into planning or `AGENTS.md`.
- Replaced `spec/readme-first-use.md` with nine section-bounded checks. They
  enforce the new outline and owning-section facts while globally rejecting
  stale release history, internal evidence vocabulary, maintainer commands,
  and internal planning/contract links.
- `mustmatch test -v spec/readme-first-use.md`: 9 passed.
- `make spec`: 275 passed, 7 skipped.
- `make lint`: passed (`cargo fmt`, Clippy with warnings denied, and dependency
  policy); only the repository's already-allowed duplicate-dependency warnings
  were reported by `cargo deny`.
- `git diff --check`: passed. `CITATION.cff`, `LICENSE`, `NOTICE`, and
  `assets/notices/` targets referenced by the README all exist. No runtime,
  asset, citation, release, or source-code file changed.
- Code-review remediation now defines `found`, `not_found`,
  `ambiguous_source_reference`, and `mixed`, including the distinction between
  no record and a zero-effect prediction and the published source-`N`
  ambiguity. It replaces the false pinned-installer update wording with
  selected-release binary/container replacement that preserves assets and
  volumes. The redundant platform section was removed and its unique lifecycle
  fact moved to HTTP; existing Linux, architecture, CPU, assembly, and exposure
  facts remain once in their owning sections.
- Remediation checks: `mustmatch test -v spec/readme-first-use.md` passed 9/9;
  `make spec` passed 275 with 7 skipped; `git diff --check` passed.

## Adversarial Code Review

Initial reviewer: Codex subagent `/root/ticket059_readme_code_review`,
2026-08-05. Verdict: REJECT. It found missing result-status meanings, false
pinned-installer update wording, and a redundant platform section. Those
findings caused the reviewed ticket revision and implementation remediation
recorded above. The original reviewer then reached its agent thread limit.

Independent remediation reviewer: Codex subagent
`/root/ticket059_remediation_code_review`, 2026-08-05. Initial verdict: REJECT
one precision phrase: `ambiguous_source_reference` called a source-associated
gene “affected,” which implied an unsupported biological conclusion.

Resolution: identify the source-associated gene, published alternate alleles,
and omitted alternate exactly, and pin that wording in the bounded spec.

Final verdict: ACCEPT. The reviewer verified the complete current diff,
six-section outline, status meanings, update/reuse paths, scientific/parser/
HTTP/Docker/storage/legal facts, compactness, and absence of internal history.
Focused README specs passed 9/9 and `git diff --check` passed. No material
finding remains.

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending

The first full `make test` final gate exposed a scope-causal stale unit
assertion: `current_state_documents_point_to_the_checked_catalog` required the
public README to contain `pangopup-build --help`, directly contradicting the
reviewed removal of maintainer commands. The test failed after all preceding
tests passed. Resolution requires removing only the README assertion while
retaining the FAQ/frontier catalog checks. This scope clarification returns to
the same ticket reviewer before implementation and code re-review; it is not a
reason to restore maintainer trivia to the user guide.

Final scope re-review verdict: ACCEPT. The ticket reviewer confirmed removing
only the README assertion and unused include is the correct narrow fix while
the FAQ/frontier assertions preserve maintainer discoverability. No build-CLI
behavior change is authorized or needed.

The second full `make test` final gate passed the repaired builder catalog test
but exposed another scope-causal stale assertion: the Ticket 057 citation test
accepted only the DOI-resolver spelling while this reviewed README uses the
user-supplied direct Springer article URL for the same DOI. Resolution: accept
either authoritative paper URL while preserving exact author, repository, and
Zenodo assertions. This narrow test correction returns to ticket and code
review; it is not a reason to replace the user's direct article link.

Citation-test scope re-review verdict: ACCEPT. The reviewer confirmed an exact
logical OR over the DOI resolver and direct Springer article URL preserves the
paper identity and user-supplied link while all creator, repository, and
Zenodo assertions remain unchanged.

Gate-remediation developer: Codex subagent
`/root/ticket059_gate_remediation`, 2026-08-05.

- Removed only the stale public-README include and assertion from
  `current_state_documents_point_to_the_checked_catalog`; the FAQ and frontier
  assertions remain unchanged, and no CLI behavior changed.
- Preserved the independently reviewed README and bounded first-use spec diff.
- Focused `pangopup-build` catalog test passed 1/1; the bounded README spec
  passed 9/9; `git diff --check` passed. The first exact-filter invocation
  selected no tests because the unit test is compiled under the binary target;
  rerunning with the unqualified name selected and passed the intended test.

Final gate-remediation code review: ACCEPT by
`/root/ticket059_remediation_code_review`. The reviewer confirmed the source
diff removes only the obsolete README include/assertion, retains FAQ/frontier
maintainer checks, changes no CLI behavior, and leaves the accepted README and
spec facts intact. No material finding remains.
