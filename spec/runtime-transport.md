# Derived runtime-asset local transport

The maintainer command packages only the model, compact reference, and splice
mask. The miniature profile deliberately names a fictional 15 GB SNV member:
packing succeeds without that member being present because SNV identity is
metadata only.

```bash
rm -rf ../target/spec/runtime-transport
mkdir -p ../target/spec/runtime-transport
pangopup-build runtime-transport pack \
  --profile ../tests/fixtures/runtime-transport-mini/runtime-profile.json \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/gencode-mask-mini/domains.pgm \
  --output ../target/spec/runtime-transport/first \
  | mustmatch like '{"command":"runtime-transport.pack","compressed_bytes":654,"runtime_profile_id":"sha256:ea178659923ab4dfc7e0cb88f55b129d994ebad42be4e9dcae76f16f03794940","status":"ok","transport_id":"sha256:0e373cf2183312d8f6f28b286aa49ad2395ea5922bf650e0f15b536908d45f6c"}'
pangopup-build runtime-transport pack \
  --profile ../tests/fixtures/runtime-transport-mini/runtime-profile.json \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/gencode-mask-mini/domains.pgm \
  --output ../target/spec/runtime-transport/second >/dev/null
find ../target/spec/runtime-transport/first -type f -print | sed 's|.*/||' | sort | mustmatch like "domains.pgm.zst
mask-NOTICE
model-NOTICE
model-manifest.json
model.onnx.zst
reference-NOTICE
reference-manifest.json
reference.pgr.zst
runtime-profile.json
runtime-transport.json"
diff -qr ../target/spec/runtime-transport/first ../target/spec/runtime-transport/second
```

Verification streams all three compressed frames and all bounded metadata but
creates no reconstructed output. Unpack performs one decode into a private
stage and atomically publishes byte-exact runtime inputs.

```bash
pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/first \
  | mustmatch like '{"command":"runtime-transport.verify","compressed_bytes":654,"runtime_profile_id":"sha256:ea178659923ab4dfc7e0cb88f55b129d994ebad42be4e9dcae76f16f03794940","status":"ok","transport_id":"sha256:0e373cf2183312d8f6f28b286aa49ad2395ea5922bf650e0f15b536908d45f6c"}'
test ! -e ../target/spec/runtime-transport/verified-output
pangopup-build runtime-transport unpack \
  --transport ../target/spec/runtime-transport/first \
  --output ../target/spec/runtime-transport/unpacked \
  | mustmatch like '{"command":"runtime-transport.unpack","runtime_profile_id":"sha256:ea178659923ab4dfc7e0cb88f55b129d994ebad42be4e9dcae76f16f03794940","status":"ok","transport_id":"sha256:0e373cf2183312d8f6f28b286aa49ad2395ea5922bf650e0f15b536908d45f6c"}'
cmp ../target/spec/runtime-transport/unpacked/runtime-profile.json ../tests/fixtures/runtime-transport-mini/runtime-profile.json
cmp ../target/spec/runtime-transport/unpacked/model/model.onnx ../tests/fixtures/pangolin-model-kernel-mini/bundle/model.onnx
cmp ../target/spec/runtime-transport/unpacked/reference/reference.pgr ../tests/fixtures/reference-route-test/bundle/reference.pgr
cmp ../target/spec/runtime-transport/unpacked/mask/domains.pgm ../tests/fixtures/gencode-mask-mini/domains.pgm
```

The transport is a closed ten-file set. Extra, substituted, truncated,
corrupted, symlinked, and non-regular members fail before an unpacked
destination can be published.

```bash run id=runtime-transport-extra exit=1 stream=stderr
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/extra
touch ../target/spec/runtime-transport/extra/unexpected
pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/extra
```

```text expect=runtime-transport-extra exact
{"status":"error","code":"PART_SET_INVALID","message":"runtime transport directory member set mismatch","details":null}
```

```bash run id=runtime-transport-truncated exit=1 stream=stderr
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/truncated
truncate -s -1 ../target/spec/runtime-transport/truncated/reference.pgr.zst
pangopup-build runtime-transport unpack --transport ../target/spec/runtime-transport/truncated --output ../target/spec/runtime-transport/not-published
```

```text expect=runtime-transport-truncated exact
{"status":"error","code":"PART_SET_INVALID","message":"compressed member size does not match manifest","details":null}
```

```bash
test ! -e ../target/spec/runtime-transport/not-published
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/corrupt
printf X >> ../target/spec/runtime-transport/corrupt/model.onnx.zst
! pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/corrupt
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/substituted
printf changed > ../target/spec/runtime-transport/substituted/model-NOTICE
! pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/substituted
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/symlinked
rm ../target/spec/runtime-transport/symlinked/mask-NOTICE
ln -s model-NOTICE ../target/spec/runtime-transport/symlinked/mask-NOTICE
! pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/symlinked
cp -R ../target/spec/runtime-transport/first ../target/spec/runtime-transport/fifo
rm ../target/spec/runtime-transport/fifo/model-NOTICE
mkfifo ../target/spec/runtime-transport/fifo/model-NOTICE
! pangopup-build runtime-transport verify --transport ../target/spec/runtime-transport/fifo
test ! -e ../target/spec/runtime-transport/not-published
```

An occupied output is never replaced. The exact flag grammar fails before
opening any supplied path.

```bash run id=runtime-transport-conflict exit=1 stream=stderr
mkdir ../target/spec/runtime-transport/occupied
touch ../target/spec/runtime-transport/occupied/sentinel
pangopup-build runtime-transport unpack --transport ../target/spec/runtime-transport/first --output ../target/spec/runtime-transport/occupied
```

```text expect=runtime-transport-conflict exact
{"status":"error","code":"OUTPUT_CONFLICT","message":"output already exists","details":null}
```

```bash run id=runtime-transport-usage exit=2 stream=stderr
pangopup-build runtime-transport pack --profile /secret/not-opened
```

```text expect=runtime-transport-usage exact
{"status":"error","code":"CLI_USAGE","message":"runtime-transport pack requires --profile, --model-bundle, --reference-bundle, --mask, and --output exactly once","details":null}
```
