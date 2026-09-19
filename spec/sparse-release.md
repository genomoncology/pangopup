# Sparse SNV release preparation

Sparse release construction is a separate two-step local command. Its public assembly path trusts only the checked production v1 bundle identity. Candidate commits use exact lowercase Git object syntax but remain construction facts until later retained-data evidence binds their output. Preparation requires its tooling commit to equal the executable's compiled Git commit from a clean checkout. It records the separately supplied release target only as release metadata.

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
