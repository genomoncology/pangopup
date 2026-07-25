# 016 — Remove the closed reference-format experiment

Status: ready

## Why

Ticket 010 compared three RefSeq encodings, Ticket 011 turned the selected
two-bit encoding into the separate production `PGRREF01` format, and the
production reader, builder, synthetic 25-contig fixture, and retained full
asset now stand on their own. The original candidate writer, three-codec
reader, benchmark executable, benchmark target, miniature candidate set, and
CLI/spec are more than 7,900 lines of completed experiment machinery. They
still compile and make the ordinary gate exercise an interface that users do
not need.

Remove that closed experiment before model work. Preserve the selected
production implementation and its evidence so Pangopup still provides exactly
the narrow RefSeq sequence capability needed by model inference.

## Scope

- Delete the closed reference-format experiment:
  - `crates/pangopup-build/src/reference_candidates.rs`;
  - `crates/pangopup-index/src/reference_candidates.rs`;
  - `crates/pangopup-build/src/bin/pangopup-reference-benchmark.rs`;
  - `crates/pangopup-index/benches/reference_formats.rs`;
  - `crates/pangopup-build/tests/reference_candidates.rs`;
  - `crates/pangopup-index/tests/reference_benchmark_structure.rs`;
  - `spec/reference-candidates.md`; and
  - `tests/fixtures/reference-candidates-mini/`.
- Remove only the corresponding module exports, Cargo feature/bin/bench
  declarations, Makefile/spec wiring, `reference-candidates prepare|inspect`
  build-CLI adapter, and the obsolete production-identity assertion that reads
  the deleted benchmark executable.
- Remove the opt-in `PANGOPUP_REFERENCE_HEAP_REPORT` allocator/reporting path
  from the ordinary build CLI and its duplicate CLI test. Keep the direct
  bounded-memory and zero-allocation coverage in
  `crates/pangopup-build/tests/reference_resources.rs`.
- Preserve byte-for-byte:
  - production codec/reader `crates/pangopup-index/src/reference.rs`;
  - production builder/provider logic `crates/pangopup-build/src/reference.rs`;
  - `tests/fixtures/reference-production-mini/`;
  - the upstream compatibility corpus;
  - the retained full production asset under
    `/home/ian/workspace/data/pangopup-reference-production-011/`; and
  - historical ADRs and `planning/artifacts/010-*` / `011-*` evidence.
- Do not rebuild, copy, recompress, fully hash, publish, install, or otherwise
  modify the 772 MB retained production member.
- Do not remove the qualification evaluator embedded in the production
  reference source, or its audited held-descriptor test API, in this ticket.
  That source participates in the recorded production builder fingerprint.
  Removing dead evaluator code belongs after a separately reviewed
  provenance-decoupling change; this ticket must not silently change the
  production identity.
- Do not add model inference, reference delivery, mask delivery, HTTP, cache,
  or new public behavior.
- Update current descriptions in `README.md`, `architecture/README.md`,
  `architecture/design.md`, `architecture/delivery.md`,
  `architecture/reference.md`, `planning/faq.md`, and `planning/frontier.md`.
  Add one negative acceptance case to the surviving `spec/reference.md`
  proving that the removed command reaches the ordinary closed CLI grammar.
  Historical decisions, reports, and reviews continue to describe what was
  run at the time and are not rewritten.

## Success Checklist

- `reference_candidates`, `reference-candidates`,
  `reference-qualification`, `pangopup-reference-benchmark`,
  `reference_formats`, and `PANGOPUP_REFERENCE_HEAP_REPORT` are absent from
  compiled/build surfaces, and the obsolete dedicated spec is removed:

  ```text
  ! rg -n 'reference_candidates|reference-candidates|reference-qualification|pangopup-reference-benchmark|reference_formats|PANGOPUP_REFERENCE_HEAP_REPORT' \
    Makefile crates
  test ! -e spec/reference-candidates.md
  ```

- Cargo metadata no longer exposes the experiment feature, binary, or
  benchmark:

  ```text
  cargo metadata --locked --no-deps --format-version 1 \
    > target/ticket016-cargo-metadata.json
  ! rg -q '"name":"pangopup-reference-benchmark"|"name":"reference_formats"|"reference-qualification"' \
    target/ticket016-cargo-metadata.json
  ```

- `spec/reference.md` proves that invoking `pangopup-build
  reference-candidates` exits 2 with the ordinary `CLI_USAGE` error on stderr,
  rather than reaching a retained private adapter.
- The production reference member, builder/provider modules, synthetic
  production fixture, and compatibility corpus remain present. The checked
  reference tests prove exact window behavior, malformed/failure behavior,
  cheap open, caller-buffer use, concurrency, bounded builder resources, and
  zero-allocation warmed windows:

  ```text
  cargo test --locked -p pangopup-index reference
  cargo test --locked -p pangopup-build \
    --test reference --test reference_resources --test builder_provenance
  cargo test --locked -p pangopup-build --test compatibility
  ```

- The retained external production bundle opens through the unchanged
  production inspector without a rebuild or payload-wide hash:

  ```text
  cargo run --locked -q -p pangopup-build --bin pangopup-build -- reference inspect \
    --bundle /home/ian/workspace/data/pangopup-reference-production-011/1303432912d9ddf9d56805d8e5956553340fa499b3b252dae44f8218fd815b01/bundle
  ```

  The response identifies format `pangopup.reference.acgt2-rle.v1`, profile
  `refseq-grch38p14-primary-v1`, the existing manifest-derived bundle ID, 25
  sequences, and 3,088,286,401 bases. It must also say
  `member_sha256_checked:false`: this cheap check does not authenticate
  `reference.pgr` or claim a member digest.
- Focused artifact-fingerprint tests prove the SNV and production-reference
  source identities did not change:

  ```text
  cargo test --locked -p pangopup-build source_fingerprint_
  ```

  No production member is re-hashed to prove a source-only deletion.
- The diff removes the seven named source/test/spec files plus the complete
  miniature candidate fixture (at least 7,900 source lines and 59,822 fixture
  bytes) and introduces no replacement experiment framework.
- Current documentation distinguishes retained historical selection evidence
  from the production reference implementation and no longer claims the three
  candidate codecs, benchmark executable, CLI, or qualification lifecycle are
  live.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **What is retained?** The options are to retain all experiment machinery,
   delete all reference work, or retain only the selected production path.
   Retaining everything preserves reproducibility at the cost of a large
   compiled maintenance surface; deleting everything blocks model inference.
   Retain `PGRREF01`, its builder/provider, production fixture, external asset,
   and durable evidence; delete only the superseded comparison machinery.
2. **How is the production asset protected?** Rebuilding or fully hashing it
   would repeat expensive work without testing the change. Keep the external
   directory untouched, exercise a cheap production open, and rely on the
   unchanged production modules plus their focused exactness/resource tests.
3. **How far does dead qualification cleanup go?** Removing the standalone
   executable and CLI-only heap reporter is isolated. Editing the embedded
   evaluator in `pangopup-build/src/reference.rs` would alter a recorded
   artifact-specific source fingerprint. Stop at that coupling and handle it
   in a later explicit provenance ticket.
4. **What evidence remains?** Historical ADRs and retained reports explain why
   the chosen format exists and must not be made false by rewriting history.
   Current architecture and frontier documents describe only what exists at
   HEAD.
5. **What replaces the candidate miniature?** Nothing. The independently
   hardened 25-contig synthetic production fixture and production reader tests
   already cover the runtime contract. Keeping a second obsolete container is
   testing a discarded design, not the product.

## Dependencies

Tickets 010, 011, 013, and 015 complete.

## Notes

- Base commit: `08c34adb8e80ba99d22fa891ca49a46303640417`.
- This ticket is repository diet, not a new format decision. ADR 0009 and
  artifacts 010/011 remain the selection and retained qualification record.
- The production bundle path above is local retained evidence, not a portable
  normal-gate dependency. The focused external-open check is coordinator
  evidence and must skip clearly if that exact path is unavailable.
- `pangopup-index/test-read-audit` remains in use by production asset/resource
  tests and is not the experiment-only build feature named for removal.
- `reference_resources.rs` is the surviving direct proof for build heap,
  open/read behavior, and warmed-window allocation; do not replace the removed
  CLI heap report with another reporting framework.
- No long-running job, network request, external publication, or production
  artifact generation belongs to this ticket.

## Coordinator Authorship

Coordinator: Codex (`/root`)

Drafted from the shipped Ticket 015 outcome and the rolling repository-diet
frontier. The coordinator owns substantive ticket-review remediation but does
not implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: independent sub-agent `/root/ticket016_design_review`

First review: rejected.

1. The external-open expectation overstated structural-only inspection as
   member authentication. Corrected the contract to name only the returned
   manifest identity and structural facts, and to require
   `member_sha256_checked:false`.
2. The metadata, fingerprint, and removed-CLI checks were not exact enough.
   Added executable commands for metadata/fingerprint evidence and required
   the CLI rejection in the surviving `spec/reference.md`.
3. `planning/faq.md` had a current sentence implying the benchmark files still
   existed. Added it to the implementation/documentation scope.

First re-review: rejected because the broad stale-surface scan also searched
`spec/`, contradicting the intentional negative `reference-candidates` case in
the surviving reference spec. Restricted that scan to compiled/build surfaces
and added an exact nonexistence check for the deleted spec.

Second re-review: rejected on wording only because the prose still claimed the
intentional negative command name was absent from all specs. Corrected the
sentence to match the executable checks.

Final re-review: accepted as ready with no remaining material findings.
Accepted contract SHA-256:
`9ae6b13ae74e08da216534c69dd869c46006898dbe9b43697cc9163a58a150c4`.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
