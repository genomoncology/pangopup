# Exact model-side runtime release preparation

The public maintainer command is fixed to the reviewed production runtime
identity. Miniature successful preparation is intentionally available only to
Rust tests through a hidden test-build contract.

```bash
pangopup-build runtime-release prepare --help \
  | mustmatch like "Usage: pangopup-build runtime-release prepare --transport <DIR> --target-commit <40_LOWERCASE_HEX> --output <ABSENT_DIR>"
rm -rf ../target/spec/runtime-release
mkdir -p ../target/spec/runtime-release
pangopup-build runtime-transport pack \
  --profile ../tests/fixtures/runtime-transport-mini/runtime-profile.json \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/gencode-mask-mini/domains.pgm \
  --output ../target/spec/runtime-release/mini >/dev/null
```

The exact flag grammar is resolved before any referenced path is opened.

```bash run id=runtime-release-flags exit=2 stream=stderr
pangopup-build runtime-release prepare \
  --transport /secret/not-opened \
  --target-commit 0123456789abcdef0123456789abcdef01234567
```

```text expect=runtime-release-flags exact
{"status":"error","code":"CLI_USAGE","message":"runtime-release prepare requires --transport, --target-commit, and --output exactly once","details":null}
```

```bash run id=runtime-release-duplicate exit=2 stream=stderr
pangopup-build runtime-release prepare \
  --transport /secret/not-opened \
  --transport /secret/not-opened-again \
  --target-commit 0123456789abcdef0123456789abcdef01234567 \
  --output ../target/spec/runtime-release/not-created
```

```text expect=runtime-release-duplicate exact
{"status":"error","code":"CLI_USAGE","message":"runtime-release prepare requires --transport, --target-commit, and --output exactly once","details":null}
```

Target commits are exact lowercase 40-character hexadecimal Git object names.
Invalid targets fail before transport inspection or output creation.

```bash run id=runtime-release-target exit=1 stream=stderr
pangopup-build runtime-release prepare \
  --transport /secret/not-opened \
  --target-commit ABCDEF0123456789abcdef0123456789abcdef01 \
  --output ../target/spec/runtime-release/not-created
```

```text expect=runtime-release-target exact
{"status":"error","code":"RELEASE_INVALID","message":"target commit must be exactly 40 lowercase hexadecimal characters","details":null}
```

```bash
test ! -e ../target/spec/runtime-release/not-created
```

A structurally valid miniature transport still fails because normal CLI
success accepts only the exact retained production transport and runtime
profile identities.

```bash run id=runtime-release-nonproduction exit=1 stream=stderr
pangopup-build runtime-release prepare \
  --transport ../target/spec/runtime-release/mini \
  --target-commit 0123456789abcdef0123456789abcdef01234567 \
  --output ../target/spec/runtime-release/not-production
```

```text expect=runtime-release-nonproduction exact
{"status":"error","code":"RELEASE_INVALID","message":"runtime transport manifest identity mismatch","details":null}
```

```bash
test ! -e ../target/spec/runtime-release/not-production
test -z "$(find ../target/spec/runtime-release -maxdepth 1 -name '.not-production.pangopup-stage-*' -print -quit)"
```
