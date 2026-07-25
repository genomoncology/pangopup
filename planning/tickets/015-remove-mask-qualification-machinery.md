# 015 — Remove obsolete GENCODE mask qualification machinery

Status: complete
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
- Independent design reviewer: `/root/ticket015_design_review`.
- Implementer: `/root/ticket015_implement`; differs from the design reviewer.
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

Developer: `/root/ticket015_implement`.

The developer verified reviewed commit
`418246dc129e1f5fcfadbb3f830faaaac5413eaa`, base
`34a69c3d3f97741dc726f47378f1d7b27670805b`, contract identity
`4cb18507d3bf058e6ab77453574fbbab833e202602bd041975f7ff0614a61c44`,
and a clean pre-implementation ownership map.

Before deleting the accepted writer, a temporary
`pangopup-index` example parsed only the existing miniature's `genes` input and
called `write_mask_candidate(..., MaskCandidateCodec::Domains, ...)`. Both
destinations were first proved absent. The exact two generation commands were:

```text
cargo run --locked --quiet -p pangopup-index --example generate_mask_domains_fixture -- tests/fixtures/gencode-mask-mini/fixture.json tests/fixtures/gencode-mask-mini/domains.pgm
cargo run --locked --quiet -p pangopup-index --example generate_mask_domains_fixture -- tests/fixtures/gencode-mask-mini/fixture.json /tmp/pangopup-ticket015-domains-second.pgm
```

Both commands printed `880`; this command succeeded with no difference:

```text
cmp tests/fixtures/gencode-mask-mini/domains.pgm /tmp/pangopup-ticket015-domains-second.pgm
```

The committed miniature is exactly 880 bytes with SHA-256
`76d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43`.
The temporary generator was removed before review. Production-mask tests now
deserialize only `fixture.json`'s independently authored `queries`; they open
the pinned binary directly, pin its byte count/digest, mutate copies for
corruption controls, and change header byte 10 to `1` and `3` for rejected
codec controls. The allocation test also opens the pinned binary directly.

The five named mask candidate/qualification source/spec files, feature, binary
target, crate-root modules, `make spec` build, mask-wide build-script hash, and
Ticket 012 frozen hash assertion were removed. The artifact-specific SNV and
reference inventories, algorithms, domains, and expected identities were not
changed; the focused source-fingerprint suite passed 19 tests, including all
four integration invariance controls. Current documentation now distinguishes
the production domains reader and retained evidence from the deleted one-time
machinery.

Focused evidence:

```text
cargo test --locked -p pangopup-index --test mask --test mask_allocations --test mask_retained_member
# 6 passed; 1 retained-member test ignored as designed

PANGOPUP_MASK_MEMBER=/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/prepare/candidates/domains.pgm \
  cargo test --locked -p pangopup-index --test mask_retained_member \
  -- --ignored --exact retained_domains_member_matches_oracle
# 1 passed; all 1,000 JCS+LF hashes matched

cargo test --locked -p pangopup-build source_fingerprint
# 19 passed; 25 filtered out

cargo clippy --locked -p pangopup-index --tests -- -D warnings
# passed

! rg -n \
  'mask_candidates|mask-qualification|pangopup-mask-candidates|PANGOPUP_MASK_BUILDER_SOURCE_SHA256' \
  Makefile crates spec
# passed

git diff --check
# passed
```

The remediated implementation diff is 233 insertions and 12,810 deletions
(net -12,577 tracked lines). The miniature is the only new data file and is
below 64 KiB. No full-source input, selected production member, external data,
network, publication, or unrelated subsystem was mutated.

## Adversarial Code Review

Reviewer: `/root/ticket015_code_review`.

The first review found one material stale current-doc claim:
`planning/faq.md` still said the selected mask had no production provider and
described SQLite, gffutils, and raw GTF as current build inputs. The developer
corrected only that paragraph: it now distinguishes the shipped domains-only
provider from future asset delivery/installation and accurately records those
raw sources as one-time qualification inputs that are not runtime or current
build-crate dependencies. Focused stale-claim scanning and `git diff --check`
then passed.

The same reviewer re-read the bounded remediation, found the FAQ and ticket
evidence accurate, and recorded `ACCEPT`. The reviewer found no major,
separately scoped issue.

## Acceptance Trace

| Acceptance clause | Command or evidence | Result |
|---|---|---|
| Qualification code and surfaces removed | required negative `rg`; Cargo metadata shows only `reference-qualification` and no candidate target | pass |
| Portable production mask behavior | focused three-test-target command; pinned 880-byte SHA-256 identity; codec/corruption/allocation controls | pass |
| Exact retained 1,000-query behavior | exact environment-gated ignored-test command | pass, 1,000/1,000 |
| Existing product behavior unchanged | `make test`; source-fingerprint focused suite | pass; expected SNV/reference identities unchanged |
| Documentation and full gates | stale-claim scan; `make lint`; `make test`; `make spec`; `git diff --check` | pass; 143 executable specs |

## External Effect Evidence

Coordinator: not applicable. This ticket performs no public or irreversible
external effect.

## Coordinator Final Check

Coordinator: Codex primary agent.

The coordinator inspected every surviving code/test/configuration change, the
complete current-document diff, all deletions, and the only untracked addition.
`crates/pangopup-index/src/mask.rs`,
`mask_retained_member.rs`, `fixture.json`, both artifact-specific inventory
declarations, and the retained external member have no diff. The committed
miniature is 880 bytes, mode-safe for git, and independently matches SHA-256
`76d4513ba12fea21f509a3b61d01c90b2f503c24b139c2a50a4c08569994cc43`.
Cargo metadata and the required negative scan expose no deleted
feature/binary/module surface. There is no new framework, production data,
model, network effect, rebuild, or file above 64 KiB.

Final evidence on the independently accepted diff:

```text
PANGOPUP_MASK_MEMBER=.../prepare/candidates/domains.pgm \
  cargo test --locked -p pangopup-index --test mask_retained_member \
  -- --ignored --exact retained_domains_member_matches_oracle
# passed; 1,000/1,000 query hashes matched

cargo test --locked -p pangopup-index \
  --test mask --test mask_allocations --test mask_retained_member
# 6 passed; exact external test ignored in the portable run

cargo test --locked -p pangopup-build source_fingerprint
# 19 passed across unit and integration controls

make lint
make test
make spec
git diff --check
# all passed; 143 executable specs
```

The final tracked diff is 275 insertions and 12,811 deletions, a net removal of
12,536 lines, plus the sole 880-byte fixture. The code reviewer accepted the
only remediation before these final gates.

## Coordinator Authorship

Coordinator: Codex primary agent.
