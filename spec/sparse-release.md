# Sparse SNV release preparation

Sparse release construction is a separate two-step local command. Its public assembly path trusts only the checked production v1 bundle identity. Candidate commits use exact lowercase Git object syntax but remain construction facts until later retained-data evidence binds their output. Preparation requires its tooling commit to equal the executable's compiled Git commit from a clean checkout. It records the separately supplied release target only as release metadata.

Production code authenticates the exact checked v2 proof and profile through closed bounded parsers and independent byte identities. One internal accessor returns the inactive qualified sparse descriptor. The ordinary production selector, sync, discovery, installed runtime admission, and compiled runtime release remain v1-only. Canonical profile preparation can derive a sparse profile after independent exhaustive certification. The sparse profile still fails production admission. The checked miniature proves exact fixed/sparse lookup parity and identity derivation with the version and CPU policy held fixed. It makes no active HTTP claim.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  qualified_sparse_authority_is_exact_closed_and_separate_from_production
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  sparse_v2_parsers_reject_extensions_duplicates_noncanonical_and_crossed_versions
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  sparse_v2_semantics_reject_unsafe_integers_order_versions_and_cross_links
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  self_consistent_changed_sparse_pair_still_fails_independent_byte_pins
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  qualified_sparse_descriptor_changes_only_snv_and_derived_identities
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-assets \
  sparse_runtime_profile_preparation_certifies_real_members_before_derivation
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-index --test bundle_formats \
  fixed_and_sparse_bundles_have_identical_provider_answers
printf 'qualified sparse authority remains inactive\n' | mustmatch like 'qualified sparse authority remains inactive'
```

```bash
pangopup-build sparse-release assemble --help | mustmatch like 'sparse-release assemble --authority <FIXED_V1_BUNDLE> --sparse <SPARSE_PGI> --candidate-commit <40_LOWERCASE_HEX> --output <ABSENT_DIR>'
pangopup-build sparse-release prepare --help | mustmatch like 'sparse-release prepare --transport <DIR> --tooling-commit <40_LOWERCASE_HEX> --release-target-commit <40_LOWERCASE_HEX> --output <ABSENT_DIR>'
```

Malformed commit input fails before any named input is opened or output is created.

```bash run id=sparse-assemble-commit-rejected exit=1 stream=stderr
rm -rf ../target/spec/sparse-release
mkdir -p ../target/spec/sparse-release
pangopup-build sparse-release assemble \
  --authority ../target/spec/sparse-release/missing-authority \
  --sparse ../target/spec/sparse-release/missing-sparse \
  --candidate-commit NOT-A-COMMIT \
  --output ../target/spec/sparse-release/must-not-exist
```

```text expect=sparse-assemble-commit-rejected like
{"status":"error","code":"SPARSE_CANDIDATE_COMMIT","message":"candidate commit must be 40 lowercase hexadecimal characters","details":null}
```

```bash run id=sparse-prepare-commit-rejected exit=1 stream=stderr
pangopup-build sparse-release prepare \
  --transport ../target/spec/sparse-release/missing-transport \
  --tooling-commit NOT-A-COMMIT \
  --release-target-commit 1111111111111111111111111111111111111111 \
  --output ../target/spec/sparse-release/must-not-exist
```

```text expect=sparse-prepare-commit-rejected like
{"status":"error","code":"SPARSE_RELEASE_COMMIT","message":"tooling commit must be 40 lowercase hexadecimal characters","details":null}
```

```bash
test ! -e ../target/spec/sparse-release/must-not-exist
test -z "$(find ../target/spec/sparse-release -maxdepth 1 -name '.must-not-exist.pangopup-stage-*' -print -quit)"
printf 'sparse release rejection is atomic\n' | mustmatch like 'sparse release rejection is atomic'
```
