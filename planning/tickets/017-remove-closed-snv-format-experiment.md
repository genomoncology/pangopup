# 017 — Remove the closed SNV-format experiment

Status: ready

## Why

Ticket 002 compared fixed-v1 against hierarchical direct, independently
compressed Zstd/LZ4 blocks, and Tabix. Tickets 003–008 then hardened,
certified, regression-tested, transported, published, and synchronized the
selected fixed 11-byte product format. The discarded comparison still compiles
in every all-target gate through 2,240 lines of benchmark/test code and keeps
candidate-only LZ4 and noodles/Tabix dependency families in the workspace.

Remove that completed experiment before CPU model work. Preserve the production
fixed-v1 reader, writer, 1,000-SNV exact regression, performance harnesses, and
retained decision/report evidence.

## Scope

- Delete:
  - `crates/pangopup-index/benches/support/candidates.rs`;
  - `crates/pangopup-index/benches/index_formats.rs`; and
  - `crates/pangopup-build/tests/index_candidates.rs`.
- Remove the `index_formats` Cargo benchmark target.
- Remove dependencies that are no longer used after those files disappear:
  - candidate-only `lz4_flex`, `noodles-bgzf`, `noodles-csi`, and
    `noodles-tabix` declarations from both affected crates;
  - the now-redundant `memmap2` build-crate dev dependency; and
  - the now-redundant index-crate dev-only `libc` and `zstd` declarations.
  Regenerate `Cargo.lock` from the edited manifests without a broad dependency
  update. The lock diff may delete unreachable packages and dependency edges
  only; it must not change the version, source, or checksum of any surviving
  package.
- Preserve byte-for-byte:
  - `crates/pangopup-index/src/snv.rs`;
  - `crates/pangopup-build/src/snv.rs`;
  - `crates/pangopup-build/src/production.rs`;
  - the checked ingestion and 1,000-SNV regression fixtures;
  - production lookup/serialization benchmarks in `pangopup-cli`; and
  - accepted ADRs plus `planning/artifacts/002-*`, `003-*`, and `004-*`.
- Do not rebuild, verify, hash, repack, download, install, or publish the
  production SNV asset.
- Do not change the fixed-v1 format, query API, CLI output, source ingestion,
  transport, XDG installation, model/reference/mask behavior, or HTTP plans.
- Do not remove the artifact-specific source fingerprint, the unused legacy
  build-script fingerprint, or dead reference qualification evaluator here.
  Those unrelated maintenance surfaces do not block model inference and no
  further cleanup ticket is implied.
- Update current descriptions in `README.md`, `architecture/index.md`,
  `planning/faq.md`, and `planning/frontier.md`. Historical ADRs, reports,
  issues, and reviews continue to describe the experiments actually run and
  are not rewritten.

## Success Checklist

- The three named experiment files and the `index_formats` benchmark target no
  longer exist:

  ```text
  test ! -e crates/pangopup-index/benches/support/candidates.rs
  test ! -e crates/pangopup-index/benches/index_formats.rs
  test ! -e crates/pangopup-build/tests/index_candidates.rs
  cargo metadata --locked --no-deps --format-version 1 \
    > target/ticket017-cargo-metadata.json
  ! rg -q '"name":"index_formats"' target/ticket017-cargo-metadata.json
  ```

- Candidate-only dependencies are absent from current crate manifests and the
  workspace dependency graph. The crate-specific metadata checks also prove
  that the three redundant declarations whose package names remain legitimate
  elsewhere were removed from the correct `[dev-dependencies]` sections:

  ```text
  ! rg -n 'lz4_flex|noodles-bgzf|noodles-csi|noodles-tabix' \
    crates --glob Cargo.toml
  jq -e '
    [.packages[]
      | select(.name == "pangopup-build")
      | .dependencies[]
      | select(.kind == "dev")
      | .name]
    | (index("memmap2") == null)
  ' target/ticket017-cargo-metadata.json > /dev/null
  jq -e '
    [.packages[]
      | select(.name == "pangopup-index")
      | .dependencies[]
      | select(.kind == "dev")
      | .name]
    | (index("libc") == null and index("zstd") == null)
  ' target/ticket017-cargo-metadata.json > /dev/null
  cargo tree --locked --workspace -e all > target/ticket017-cargo-tree.txt
  ! rg -q 'lz4_flex|noodles-bgzf|noodles-csi|noodles-tabix' \
    target/ticket017-cargo-tree.txt
  ```

- The `Cargo.lock` diff against the base commit contains deletions only. Cargo
  prunes unreachable candidate packages and edges without upgrading or
  replacing a surviving dependency:

  ```text
  git diff --unified=0 \
    7d177db144b200bbd7861a0fcc87d43931f1f790 -- Cargo.lock \
    > target/ticket017-cargo-lock.diff
  ! sed -n '/^+++ /d; /^+/p' target/ticket017-cargo-lock.diff \
    | grep -q .
  ```

- The three production SNV source modules named in scope are byte-identical to
  base `7d177db144b200bbd7861a0fcc87d43931f1f790`.
- Focused artifact-specific fingerprint tests prove the SNV and reference
  identities remain unchanged:

  ```text
  cargo test --locked -p pangopup-build source_fingerprint_
  ```

- Production exactness is proved by the surviving inside-out tests, including
  the real 1,000-request oracle:

  ```text
  cargo test --locked -p pangopup-index snv
  cargo test --locked -p pangopup-build \
    --test full_bundle --test snv_regression_fixture
  cargo test --locked -p pangopup-cli --test snv_regression
  ```

- Current documentation says the comparison implementations were removed while
  retained evidence explains the fixed-v1 selection. It does not imply that
  users can rerun a benchmark target that no longer exists.
- The implementation removes at least the audited 2,240 Rust lines and does
  not introduce a replacement candidate framework or fixture.
- `make lint`, `make test`, and `make spec` pass.

## Decisions

1. **Delete or retain the candidate harness?** Retaining it preserves an easy
   rerun but makes all-target builds compile discarded codecs and dependencies.
   The selected product has separate writer/reader/regression/performance
   coverage and the exact historical run is retained. Delete the harness.
2. **What happens to comparative evidence?** Rewriting old ADRs and reports
   would erase why fixed-v1 was selected. Keep them immutable; update only
   current architecture and planning claims.
3. **Does this justify touching production data?** No query or artifact byte
   changes. Byte-identical source checks, artifact-specific fingerprints, and
   the fast 1,000-case oracle are discriminating; a multi-gigabyte asset hash or
   rebuild would add no relevant evidence.
4. **How far does dependency cleanup go?** Remove declarations made unused by
   these exact files and let Cargo prune their unreachable lock entries. Do not
   opportunistically upgrade or reorganize surviving dependencies.
5. **What comes next?** CPU inference. Known dead legacy provenance/evaluator
   code is not on the runtime path and does not warrant another pre-model
   repository-diet slice.

## Dependencies

Tickets 002–008, 013, and 016 complete.

## Notes

- Base commit: `7d177db144b200bbd7861a0fcc87d43931f1f790`.
- The candidate harness is excluded from both artifact-specific source
  inventories. Removing its files and dev dependencies must not change either
  production fingerprint.
- `tests/fixtures/pangolin-precompute/` is shared by production ingestion and
  regression work; it is not an experiment-only fixture and must remain.
- `memmap2`, `libc`, and `zstd` remain legitimate dependencies elsewhere.
  Only the redundant declarations named in scope are removed.
- No long-running job, external asset read, network request, publication, or
  generated production artifact belongs to this ticket.

## Coordinator Authorship

Coordinator: Codex (`/root`)

Drafted from the shipped Ticket 016 outcome, the rolling core-first plan, and
an independent read-only audit of the remaining compiled format experiments.
The coordinator owns substantive ticket-review remediation but does not
implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: Mencius the 2nd (`/root/ticket017_design_review`)

Accepted after one remediation. The initial review found that the dependency
acceptance checks could miss redundant `memmap2`, `libc`, and `zstd`
dev-dependency declarations because those packages correctly remain elsewhere.
The coordinator added crate-specific metadata assertions plus a deletion-only
lockfile-diff requirement. The reviewer reran those controls, confirmed they
fail against the unmodified base, simulated the intended offline resolution,
and accepted the amended contract with no remaining findings.

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
