# Pinned SNV public-release preparation

The reviewed profile is canonical JSON with no trailing newline. Its proof
receipt is the exact reviewed 2,193-byte canonical JSON prefix plus one LF;
the whole 2,194-byte file has the pinned public identity.

```bash
test "$(stat -c %s ../release-profiles/proofs/snv-grch38-v1.json)" = 2194
test "$(sha256sum ../release-profiles/proofs/snv-grch38-v1.json | cut -d' ' -f1)" = 9ddae771d200fe73bda5f31f5a04a52227b77c5d3f225dc7ee52294cd9aea475
test "$(tail -c 1 ../release-profiles/proofs/snv-grch38-v1.json | od -An -t x1 | tr -d ' ')" = 0a
test "$(tail -c 1 ../release-profiles/snv-grch38-v1.json)" = '}'
printf 'pinned release metadata framing\n' | mustmatch like 'pinned release metadata framing'
```

The release command grammar is closed and checked before filesystem work.

```bash run id=release-action-usage exit=2 stream=stderr
pangopup-build release publish
```

```text expect=release-action-usage like
{"status":"error","code":"CLI_USAGE","message":"release requires prepare or upload-asset","details":null}
```

```bash run id=release-flags-usage exit=2 stream=stderr
pangopup-build release prepare --transport /tmp/unused --receipt /tmp/unused --unknown /tmp/unused
```

```text expect=release-flags-usage like
{"status":"error","code":"CLI_USAGE","message":"release prepare requires --transport, --receipt, and --output exactly once","details":null}
```

The coordinator-only uploader also has closed grammar and rejects an invalid
release ID or a non-absolute executable path before touching either asset
root.

```bash run id=release-upload-id-usage exit=2 stream=stderr
pangopup-build release upload-asset \
  --transport /tmp/unused \
  --prepared /tmp/unused \
  --gh /tmp/unused-gh \
  --release-id 0 \
  --asset transport.json
```

```text expect=release-upload-id-usage like
{"status":"error","code":"CLI_USAGE","message":"release upload-asset requires a positive decimal --release-id","details":null}
```

```bash run id=release-upload-gh-usage exit=1 stream=stderr
pangopup-build release upload-asset \
  --transport /tmp/unused \
  --prepared /tmp/unused \
  --gh relative-gh \
  --release-id 1 \
  --asset transport.json
```

```text expect=release-upload-gh-usage like
{"status":"error","code":"RELEASE_UPLOAD","message":"GitHub CLI path must be absolute","details":null}
```

The public CLI rejects any other receipt contract before inspecting a
transport or creating output. Failure is one deterministic JSON line on stderr
and leaves no stage.

```bash run id=release-contract-rejected exit=1 stream=stderr
rm -rf ../target/spec/snv-release
mkdir -p ../target/spec/snv-release
printf '{}\n' > ../target/spec/snv-release/other-receipt.json
pangopup-build release prepare \
  --transport ../target/spec/snv-release/no-transport \
  --receipt ../target/spec/snv-release/other-receipt.json \
  --output ../target/spec/snv-release/must-not-exist
```

```text expect=release-contract-rejected like
{"status":"error","code":"RELEASE_INVALID","message":"supplied proof receipt does not match the reviewed release contract","details":null}
```

```bash
test ! -e ../target/spec/snv-release/must-not-exist
test -z "$(find ../target/spec/snv-release -maxdepth 1 -name '.must-not-exist.pangopup-stage-*' -print -quit)"
printf 'release rejection is atomic\n' | mustmatch like 'release rejection is atomic'
```

Successful miniature preparation is exercised through the internal injected
contract seam in the normal Rust gate. It emits exactly four small files; the
release assets themselves remain the five inspected transport members plus
the proof, profile, and checksum list. The notes are the release body, not a
ninth asset.

```bash
rg -F '"proof-receipt.json"' ../release-profiles/snv-grch38-v1.json >/dev/null
rg -F '"payload.pgi.zst.part0000"' ../release-profiles/snv-grch38-v1.json >/dev/null
rg -F '"payload.pgi.zst.part0001"' ../release-profiles/snv-grch38-v1.json >/dev/null
printf 'release output contract is pinned\n' | mustmatch like 'release output contract is pinned'
```
