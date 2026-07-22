# Deterministic local SNV transport

The maintenance CLI packs a certified bundle without changing its installed
bytes. The two metadata members remain byte-exact and only the score member is
compressed into one checksummed Zstandard frame. The miniature fixture uses
one part; production uses the same exact 1,000,000,000-byte split boundary.

```bash
chmod -R u+w ../target/spec/snv-transport 2>/dev/null || true
rm -rf ../target/spec/snv-transport
mkdir -p ../target/spec/snv-transport/source
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000001.tsv > ../target/spec/snv-transport/source/ENSG00000000001.tsv.gz
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000002.tsv > ../target/spec/snv-transport/source/ENSG00000000002.tsv.gz
cp ../tests/fixtures/full-build-reference.fa ../target/spec/snv-transport/reference.fa
pangopup-build build --source ../target/spec/snv-transport/source --reference ../target/spec/snv-transport/reference.fa --output ../target/spec/snv-transport/bundle >/dev/null
pangopup-build transport pack --output ../target/spec/snv-transport/first --bundle ../target/spec/snv-transport/bundle | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g; s/"compressed_bytes":[0-9]+/"compressed_bytes":0/' | mustmatch like '{"status":"packed","transport_id":"sha256:<digest>","bundle_id":"sha256:<digest>","part_count":1,"compressed_bytes":0}'
pangopup-build transport pack --bundle ../target/spec/snv-transport/bundle --output ../target/spec/snv-transport/second >/dev/null
diff -qr ../target/spec/snv-transport/first ../target/spec/snv-transport/second
find ../target/spec/snv-transport/first -mindepth 1 -maxdepth 1 -type f -printf '%f\n' | sort | mustmatch like "NOTICE
bundle-manifest.json
payload.pgi.zst.part0000
transport.json"
```

Integrity verification is streaming and produces one stable JSON object.
Unpack certifies the reconstructed fixed-v1 bundle before atomic publication,
and every installed member is byte-identical to the input.

```bash
pangopup-build transport verify --transport ../target/spec/snv-transport/first | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g; s/"compressed_bytes":[0-9]+/"compressed_bytes":0/' | mustmatch like '{"status":"verified","transport_id":"sha256:<digest>","bundle_id":"sha256:<digest>","part_count":1,"compressed_bytes":0}'
pangopup-build transport unpack --output ../target/spec/snv-transport/unpacked --transport ../target/spec/snv-transport/first | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' | mustmatch like '{"status":"unpacked","transport_id":"sha256:<digest>","bundle_id":"sha256:<digest>"}'
cmp ../target/spec/snv-transport/bundle/NOTICE ../target/spec/snv-transport/unpacked/NOTICE
cmp ../target/spec/snv-transport/bundle/manifest.json ../target/spec/snv-transport/unpacked/manifest.json
cmp ../target/spec/snv-transport/bundle/scores.pgi ../target/spec/snv-transport/unpacked/scores.pgi
pangopup-build verify ../target/spec/snv-transport/unpacked >/dev/null
printf 'byte-exact certified reconstruction\n' | mustmatch like 'byte-exact certified reconstruction'
```

An existing output is never inspected as staging or replaced.

```bash run id=transport-conflict exit=1 stream=stderr
pangopup-build transport unpack --transport ../target/spec/snv-transport/first --output ../target/spec/snv-transport/unpacked
```

```text expect=transport-conflict contains
{"status":"error","code":"OUTPUT_CONFLICT"
```

A changed part fails at its declared integrity layer and no output directory is
published.

```bash run id=transport-corruption exit=1 stream=stderr
cp -a ../target/spec/snv-transport/first ../target/spec/snv-transport/corrupt
printf x | dd of=../target/spec/snv-transport/corrupt/payload.pgi.zst.part0000 bs=1 seek=20 count=1 conv=notrunc status=none
pangopup-build transport unpack --transport ../target/spec/snv-transport/corrupt --output ../target/spec/snv-transport/corrupt-output
```

```text expect=transport-corruption contains
{"status":"error","code":"TRANSPORT_HASH_MISMATCH"
```

```bash
test ! -e ../target/spec/snv-transport/corrupt-output
printf 'no partial publication\n' | mustmatch like 'no partial publication'
```

The manifest object is validated before a supported part set exists. A
symlinked or non-regular `transport.json` therefore fails as
`MANIFEST_INVALID`, writes nothing to stdout, and returns exit 1.

```bash run id=transport-manifest-symlink exit=1 stream=stderr
cp -a ../target/spec/snv-transport/first ../target/spec/snv-transport/manifest-symlink
unlink ../target/spec/snv-transport/manifest-symlink/transport.json
ln -s ../first/transport.json ../target/spec/snv-transport/manifest-symlink/transport.json
set +e
pangopup-build transport verify --transport ../target/spec/snv-transport/manifest-symlink >../target/spec/snv-transport/manifest-symlink.stdout 2>../target/spec/snv-transport/manifest-symlink.stderr
status=$?
set -e
test "$status" -eq 1
printf '%s\n' '{"status":"error","code":"MANIFEST_INVALID","message":"required input is not a regular file","details":null}' | cmp - ../target/spec/snv-transport/manifest-symlink.stderr
printf '%s' "$(cat ../target/spec/snv-transport/manifest-symlink.stderr)" >&2
exit "$status"
```

```text expect=transport-manifest-symlink equals
{"status":"error","code":"MANIFEST_INVALID","message":"required input is not a regular file","details":null}
```

```bash run id=transport-manifest-directory exit=1 stream=stderr
cp -a ../target/spec/snv-transport/first ../target/spec/snv-transport/manifest-directory
unlink ../target/spec/snv-transport/manifest-directory/transport.json
mkdir ../target/spec/snv-transport/manifest-directory/transport.json
set +e
pangopup-build transport verify --transport ../target/spec/snv-transport/manifest-directory >../target/spec/snv-transport/manifest-directory.stdout 2>../target/spec/snv-transport/manifest-directory.stderr
status=$?
set -e
test "$status" -eq 1
printf '%s\n' '{"status":"error","code":"MANIFEST_INVALID","message":"required input is not a regular file","details":null}' | cmp - ../target/spec/snv-transport/manifest-directory.stderr
printf '%s' "$(cat ../target/spec/snv-transport/manifest-directory.stderr)" >&2
exit "$status"
```

```text expect=transport-manifest-directory equals
{"status":"error","code":"MANIFEST_INVALID","message":"required input is not a regular file","details":null}
```

```bash
test ! -s ../target/spec/snv-transport/manifest-symlink.stdout
test ! -s ../target/spec/snv-transport/manifest-directory.stdout
printf 'manifest failures leave stdout empty\n' | mustmatch like 'manifest failures leave stdout empty'
```

Once the supported manifest has parsed, a symlinked payload is instead a
declared part-set violation. This preserves the post-parse
`PART_SET_INVALID` boundary and also leaves stdout empty.

```bash run id=transport-payload-symlink exit=1 stream=stderr
cp -a ../target/spec/snv-transport/first ../target/spec/snv-transport/payload-symlink
unlink ../target/spec/snv-transport/payload-symlink/payload.pgi.zst.part0000
ln -s ../first/payload.pgi.zst.part0000 ../target/spec/snv-transport/payload-symlink/payload.pgi.zst.part0000
set +e
pangopup-build transport verify --transport ../target/spec/snv-transport/payload-symlink >../target/spec/snv-transport/payload-symlink.stdout 2>../target/spec/snv-transport/payload-symlink.stderr
status=$?
set -e
test "$status" -eq 1
printf '%s\n' '{"status":"error","code":"PART_SET_INVALID","message":"transport entries must be regular files","details":null}' | cmp - ../target/spec/snv-transport/payload-symlink.stderr
printf '%s' "$(cat ../target/spec/snv-transport/payload-symlink.stderr)" >&2
exit "$status"
```

```text expect=transport-payload-symlink equals
{"status":"error","code":"PART_SET_INVALID","message":"transport entries must be regular files","details":null}
```

```bash
test ! -s ../target/spec/snv-transport/payload-symlink.stdout
printf 'part-set failure leaves stdout empty\n' | mustmatch like 'part-set failure leaves stdout empty'
```

The command grammar is closed and checked before filesystem or platform work.

```bash run id=transport-usage exit=2 stream=stderr
pangopup-build transport verify --transport --unknown
```

```text expect=transport-usage like
{"status":"error","code":"CLI_USAGE","message":"transport verify requires --transport exactly once","details":null}
```
