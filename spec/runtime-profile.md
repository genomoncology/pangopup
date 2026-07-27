# Four-asset runtime profile maintenance contract

The runtime-profile command is production-only. Its grammar is closed and it
does not expose a synthetic success bypass.

```bash run id=runtime-profile-missing-action exit=2 stream=stderr
pangopup-build runtime-profile
```

```text expect=runtime-profile-missing-action exact
{"status":"error","code":"CLI_USAGE","message":"runtime-profile requires prepare","details":null}
```

```bash run id=runtime-profile-unknown-action exit=2 stream=stderr
pangopup-build runtime-profile inspect --snv-bundle /secret/not-opened
```

```text expect=runtime-profile-unknown-action exact
{"status":"error","code":"CLI_USAGE","message":"runtime-profile requires prepare","details":null}
```

Missing, duplicate, or unknown flags fail before any supplied path is opened.

```bash run id=runtime-profile-duplicate exit=2 stream=stderr
pangopup-build runtime-profile prepare --snv-bundle /secret/a --snv-bundle /secret/b --model-bundle /secret/model --reference-bundle /secret/reference --mask /secret/mask --output ../target/spec/runtime-profile/profile.json
```

```text expect=runtime-profile-duplicate exact
{"status":"error","code":"CLI_USAGE","message":"runtime-profile prepare requires --snv-bundle, --model-bundle, --reference-bundle, --mask, and --output exactly once","details":null}
```

The shipped command accepts only the exact production tuple. Invalid small
paths fail with a stable bounded diagnostic and leave no output.

```bash run id=runtime-profile-invalid-input exit=1 stream=stderr
rm -rf ../target/spec/runtime-profile
mkdir -p ../target/spec/runtime-profile
pangopup-build runtime-profile prepare --snv-bundle /secret/not-present --model-bundle /secret/model --reference-bundle /secret/reference --mask /secret/mask --output ../target/spec/runtime-profile/profile.json
```

```text expect=runtime-profile-invalid-input exact
{"status":"error","code":"INPUT_IO","message":"SNV bundle input failed","details":null}
```

```bash
test ! -e ../target/spec/runtime-profile/profile.json
```

An exact but non-production miniature is incompatible, rather than corrupt or
unsafe. The check happens before later paths are opened.

```bash run id=runtime-profile-incompatible exit=1 stream=stderr
pangopup-build runtime-profile prepare --snv-bundle ../tests/fixtures/snv-regression/bundle --model-bundle /secret/not-opened --reference-bundle /secret/not-opened --mask /secret/not-opened --output ../target/spec/runtime-profile/profile.json
```

```text expect=runtime-profile-incompatible exact
{"status":"error","code":"PROFILE_INCOMPATIBLE","message":"SNV bundle is not the accepted production member","details":null}
```

An extra-member directory is rejected at the fixed fourth-entry bound.

```bash run id=runtime-profile-unsafe exit=1 stream=stderr
rm -rf ../target/spec/runtime-profile/unsafe
mkdir -p ../target/spec/runtime-profile/unsafe
touch ../target/spec/runtime-profile/unsafe/{a,b,c,d,e,f}
pangopup-build runtime-profile prepare --snv-bundle ../target/spec/runtime-profile/unsafe --model-bundle /secret/not-opened --reference-bundle /secret/not-opened --mask /secret/not-opened --output ../target/spec/runtime-profile/profile.json
```

```text expect=runtime-profile-unsafe exact
{"status":"error","code":"PROFILE_UNSAFE","message":"SNV bundle is unsafe or changed","details":null}
```

Malformed bounded metadata is corrupt. Diagnostics never echo the supplied
path or file contents.

```bash run id=runtime-profile-corrupt exit=1 stream=stderr
rm -rf ../target/spec/runtime-profile/corrupt
cp -R ../tests/fixtures/snv-regression/bundle ../target/spec/runtime-profile/corrupt
printf '{}' > ../target/spec/runtime-profile/corrupt/manifest.json
pangopup-build runtime-profile prepare --snv-bundle ../target/spec/runtime-profile/corrupt --model-bundle /secret/not-opened --reference-bundle /secret/not-opened --mask /secret/not-opened --output ../target/spec/runtime-profile/profile.json
```

```text expect=runtime-profile-corrupt exact
{"status":"error","code":"PROFILE_CORRUPT","message":"SNV bundle metadata is corrupt","details":null}
```
