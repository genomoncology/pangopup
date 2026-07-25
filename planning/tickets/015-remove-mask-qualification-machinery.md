# 015 — Remove obsolete GENCODE mask qualification machinery

Status: ready
Contract identity (SHA-256 with this value set to `pending`):
`4cb18507d3bf058e6ab77453574fbbab833e202602bd041975f7ff0614a61c44`
Base revision: `34a69c3d3f97741dc726f47378f1d7b27670805b`

## Why

Ticket 012 already selected the constant-membership `domains` representation,
and Ticket 014 moved the exact selected bytes behind the self-contained
production `pangopup_index::mask` reader. The repository still compiles and
tests more than 12,000 lines of one-time machinery that captured GENCODE through
Python/gffutils, wrote and compared three codecs, managed qualification stages,
and exposed a feature-gated maintenance CLI. None of it is used to answer a
request or to open the selected mask.

Keeping that machinery makes the product harder to understand and keeps a
private experiment in every normal lint/test/spec gate. The selected external
asset, its 1,000-query oracle, and its durable comparison report are already the
facts Pangopup needs. This ticket removes the completed experiment without
rebuilding or changing those facts.

## Outcome

Pangopup retains one production domains-only mask reader and its fast exactness
tests, while the obsolete GENCODE capture, candidate writer/readers,
qualification lifecycle, feature, CLI, and executable spec are no longer
compiled or presented as supported tooling.

## Scope

### Included

- Delete:
  - `crates/pangopup-build/src/mask.rs`;
  - `crates/pangopup-build/src/bin/pangopup-mask-candidates.rs`;
  - `crates/pangopup-index/src/mask_candidates.rs`;
  - `crates/pangopup-index/tests/mask_candidates.rs`; and
  - `spec/gencode-mask-candidates.md`.
- Remove the `mask-qualification` Cargo feature, the
  `pangopup-mask-candidates` target, the public build-crate mask module, the
  index-crate candidate module, and the mask-candidate build from `make spec`.
- In `crates/pangopup-build/build.rs`, remove the complete mask hash stanza that
  emits `PANGOPUP_MASK_BUILDER_SOURCE_SHA256`. In
  `crates/pangopup-build/src/source_fingerprint.rs`, remove the frozen Ticket
  012 mask-identity assertion and replace deleted mask-candidate entries in
  `REPRESENTATIVE_EXCLUDED` with
  `crates/pangopup-index/src/mask.rs`. Keep both artifact-specific inventories,
  domains, algorithms, expected SNV/reference hashes, and legacy bundle
  compatibility unchanged.
- Add a small checked-in `domains.pgm` under
  `tests/fixtures/gencode-mask-mini/`, generated once by the current accepted
  domains writer from the existing miniature before that writer is deleted.
  Generate the same input into a second absent temporary path, prove byte
  equality with `cmp`, and record the committed fixture's exact byte count and
  SHA-256 in both the test and implementation evidence. This file is an encoded
  test input; only the independently written `fixture.json` query expectations
  define expected behavior.
- Rewrite `crates/pangopup-index/tests/mask.rs` and
  `mask_allocations.rs` to open or copy that checked binary fixture. Codec
  rejection is proved by changing only header byte 10 to discriminator `1` and
  `3`; corruption controls mutate copies of the same fixture. No writer or
  alternate decoder remains in the test dependency graph.
- Preserve `crates/pangopup-index/src/mask.rs`,
  `mask_retained_member.rs`, `fixture.json`, the exact selected local member,
  `planning/artifacts/012-performance-manifest.jsonl`, and all retained Ticket
  012 reports and historical ADRs.
- Update current-state wording in `AGENTS.md`, `README.md`,
  `architecture/README.md`, `architecture/design.md`,
  `architecture/delivery.md`, `architecture/index.md`,
  `architecture/runtime-data.md`,
  `architecture/decisions/0012-artifact-specific-builder-provenance.md`,
  `architecture/decisions/0013-byte-identical-gencode-mask-promotion.md`,
  `planning/faq.md`, and `planning/frontier.md`. Historical reports, reviews,
  issues, and ADR 0011 remain historical evidence and are not rewritten merely
  because their commands no longer exist at HEAD. ADR 0012's current
  consequence that the mask-qualification fingerprint remains must be retired;
  its accepted SNV/reference provenance decision stays unchanged.

### Excluded

- Do not change the production mask format, reader, public API, selected member
  bytes, query behavior, or performance contract.
- Do not run GTF/gffutils capture, rebuild the full mask, rewrite the selected
  member, or delete anything under `/home/ian/workspace/data/`.
- Do not remove the SNV index candidates, reference candidates, compatibility
  corpus, SNV/reference builders, their artifact-specific source fingerprints,
  release assets, sync, or local installation code.
- Do not add a replacement generic qualification framework, production mask
  builder, verifier, bundle, transport, installer, downloader, or publication
  path.
- Do not start model inference, routing, caching, HTTP, Docker, or GitHub asset
  publication.
- Do not remove dependencies unless repository-wide use proves they became
  unused as a direct consequence of this deletion.

## Decisions

1. **Delete executable history; retain durable evidence.** Git and
   `planning/artifacts/012-*` retain how the format was selected. Runtime source
   retains only what opens the selected asset. This avoids carrying a second
   product inside the repository while preserving the decision record.
2. **Use a checked binary fixture, not a test writer.** Reader tests need encoded
   input but do not need a production or candidate encoder. The existing JSON
   queries remain the independent expected behavior; a pinned tiny member
   exercises the real byte decoder without circularly generating bytes during
   each test.
3. **Remove the private executable spec.** An executable contract for a deleted
   maintenance CLI would falsely make it a supported product surface. Mask
   behavior remains covered inside-out by Rust tests and by the exact retained
   1,000-query oracle.
4. **Keep historical artifacts immutable.** Reports may contain old commands,
   fingerprints, and candidate names because they describe what ran. Current
   docs must say that source machinery was removed; historical evidence must
   not be rewritten to pretend it never existed.
5. **No model work in a cleanup diff.** This ticket establishes a smaller,
   clearer base for CPU inference. It cannot change runtime behavior merely to
   make the following ticket easier.

## Discriminating Controls

- Before the change, all five deleted files exist, the feature and binary are
  present in Cargo metadata, and `make spec` explicitly builds the private
  binary.
- After the change, this command finds no compiled qualification surface:

  ```text
  ! rg -n \
    'mask_candidates|mask-qualification|pangopup-mask-candidates|PANGOPUP_MASK_BUILDER_SOURCE_SHA256' \
    Makefile crates spec
  ```

- The miniature domains fixture has a hard expected size and SHA-256, opens
  through `MaskDomainsOpen`, and produces every independent expected query in
  `fixture.json`. The final test must not deserialize fixture genes or use them
  to derive expected results; it consumes only the pinned binary and the
  independently authored query expectations.
- Implementation evidence records the exact pre-deletion writer command,
  generated size and SHA-256, and successful byte-for-byte `cmp` against a
  second generation. The coordinator independently checks the committed
  fixture's recorded identity before final gates.
- Header discriminator mutations `1` and `3` still return
  `MaskError::UnsupportedCodec`; malformed header/directory and touched
  gene/boundary/domain/posting mutations still fail without partial output.
- The warmed 10,000-query allocation control still reports zero allocations.
- The exact retained member is not rebuilt. This read-only test must still
  match all 1,000 independent JCS+LF hashes:

  ```text
  PANGOPUP_MASK_MEMBER=/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm \
    cargo test --locked -p pangopup-index --test mask_retained_member \
    -- --ignored --exact retained_domains_member_matches_oracle
  ```

## Acceptance

- `pangopup_index::mask` is the only compiled GENCODE mask implementation.
- Cargo metadata, `make spec`, and the crate roots expose no mask qualification
  feature, binary, or candidate module.
- The checked miniature binary is below 64 KiB, has a test-pinned size/digest,
  and its semantic expectations remain independently sourced from
  `fixture.json`.
- Focused production mask tests pass:

  ```text
  cargo test --locked -p pangopup-index \
    --test mask --test mask_allocations --test mask_retained_member
  ```

  The retained-member test remains ignored in the portable command and passes
  separately with the exact command above.
- The 1,000-case SNV regression, reference provider, compatibility corpus,
  local installation, and sync behavior remain unchanged through the normal
  repository gates.
- `make lint`, `make test`, `make spec`, and `git diff --check` pass.
- The diff is deletion-dominant by more than 10,000 tracked lines, adds no
  framework, production data, model, or network effect. The miniature is the
  only newly added data/artifact, is below 64 KiB, and no other newly added file
  is larger than it; edits to existing documentation are not “newly added
  files.”
- Current docs describe the selected production reader and retained historical
  evidence without claiming that the deleted qualification executable remains
  available.

## Dependencies

- Ticket 012 selected format/member and retained evidence: complete.
- Ticket 014 independent production mask reader: complete.

## Work Ownership

- Coordinator-authored ticket and review dispositions: Codex primary agent.
- Independent design reviewer: pending.
- Implementer: pending; must differ from the design reviewer.
- Adversarial code reviewer: pending; must differ from both.
- No user-authored, generated, or unrelated working-tree changes exist at the
  recorded base.

## Long-running Jobs

None. The ticket forbids full-source capture, rebuild, benchmark, publication,
and network work.

## Independent Ticket Review

Reviewer: `/root/ticket015_design_review`.

The first review rejected three ambiguities. The coordinator then:

- made `build.rs`, `source_fingerprint.rs`, the replacement production-mask
  exclusion, and unchanged SNV/reference identities explicit;
- added ADR 0012 and retired its stale mask-fingerprint consequence;
- limited the artifact-size clause to newly added files; and
- required two pre-deletion fixture generations, byte equality, pinned
  size/SHA-256, and tests that consume only independently authored query
  expectations.

The same reviewer re-read the complete revision at base
`34a69c3d3f97741dc726f47378f1d7b27670805b`, found no remaining issue, confirmed
the 12,488-line deletion boundary is the smallest unblocked repository-diet
slice, and recorded `ACCEPTED AS READY`.

## Implementation Evidence

Developer: pending.

## Adversarial Code Review

Reviewer: pending.

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Qualification code and surfaces removed | pending | pending |
| Portable production mask behavior | pending | pending |
| Exact retained 1,000-query behavior | pending | pending |
| Existing product behavior unchanged | pending | pending |
| Documentation and full gates | pending | pending |

## External Effect Evidence

Coordinator: not applicable. This ticket performs no public or irreversible
external effect.

## Coordinator Final Check

Coordinator: pending.

## Coordinator Authorship

Coordinator: Codex primary agent.
