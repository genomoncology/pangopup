# Linux local asset installation

The runtime installs an already available Ticket 005 transport into an
isolated absolute data root. The spec uses only the checked-in miniature build
fixture and never reads a production asset.

```bash
chmod -R u+w ../target/spec/local-assets 2>/dev/null || true
rm -rf ../target/spec/local-assets
mkdir -p ../target/spec/local-assets/source
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000001.tsv > ../target/spec/local-assets/source/ENSG00000000001.tsv.gz
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000002.tsv > ../target/spec/local-assets/source/ENSG00000000002.tsv.gz
pangopup-build build --source ../target/spec/local-assets/source --reference ../tests/fixtures/full-build-reference.fa --output ../target/spec/local-assets/bundle >/dev/null
pangopup-build transport pack --bundle ../target/spec/local-assets/bundle --output ../target/spec/local-assets/transport >/dev/null
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup assets status --data-dir "$data" | sed "s|$data|<data>|" | mustmatch like '{"status":"missing","data_dir":"<data>"}'
```

Install publishes one immutable bundle and an active profile. Successful
stdout is one compact object whose path names the three-member bundle itself.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup assets install --transport ../target/spec/local-assets/transport --data-dir "$data" | sed -E "s|$data|<data>|; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"installed","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle"}'
pangopup assets status --data-dir "$data" | sed -E "s|$data|<data>|; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"ready","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle","installing":false}'
test "$(stat -c %a "$data")" = 700
test "$(stat -c %a "$data/active.json")" = 600
test "$(find "$data/bundles" -mindepth 1 -maxdepth 1 -type d -printf '%m')" = 555
test "$(find "$data/bundles" -mindepth 2 -maxdepth 2 -type d -name bundle -printf '%m')" = 555
test "$(find "$data/bundles" -type f -name scores.pgi -printf '%m')" = 444
printf 'private atomic installation\n' | mustmatch like 'private atomic installation'
```

Implicit lookup discovers the active bundle and preserves the exact existing
lookup bytes. The explicit override remains compatible and cannot be combined
with `--data-dir`.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup lookup --bundle ../target/spec/local-assets/bundle --variant GRCh38:chr1:1:A:C > ../target/spec/local-assets/explicit.jsonl
pangopup lookup --data-dir "$data" --variant GRCh38:chr1:1:A:C > ../target/spec/local-assets/implicit.jsonl
cmp ../target/spec/local-assets/explicit.jsonl ../target/spec/local-assets/implicit.jsonl
printf 'byte-identical active lookup\n' | mustmatch like 'byte-identical active lookup'
```

```bash run id=local-assets-mutual-exclusion exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/local-assets/bundle --data-dir /tmp/pangopup-unused --variant GRCh38:chr1:1:A:C
```

```text expect=local-assets-mutual-exclusion contains
{"status":"error","code":"CLI_USAGE"
```

Reuse validates installed metadata and cheap-open structure without opening a
transport part or hashing the installed score payload.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
chmod 000 ../target/spec/local-assets/transport/payload.pgi.zst.part0000
pangopup assets install --transport ../target/spec/local-assets/transport --data-dir "$data" | sed -E "s|$data|<data>|; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"reused","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle"}'
chmod 600 ../target/spec/local-assets/transport/payload.pgi.zst.part0000
```

Missing active state is a normal status result but a typed lookup failure.
Present empty or relative path configuration is invalid and never falls
through to another environment variable.

```bash run id=local-assets-missing-lookup exit=1 stream=stderr
mkdir -m 700 ../target/spec/local-assets/empty-data
PANGOPUP_DATA_DIR=$(cd .. && pwd)/target/spec/local-assets/empty-data pangopup lookup --variant GRCh38:chr1:1:A:C
```

```text expect=local-assets-missing-lookup contains
{"status":"error","code":"ASSETS_MISSING"
```

```bash run id=local-assets-relative-path exit=2 stream=stderr
PANGOPUP_DATA_DIR=relative XDG_DATA_HOME=/tmp/pangopup-unused pangopup assets status
```

```text expect=local-assets-relative-path contains
{"status":"error","code":"PATH_INVALID"
```

Install remains transactional when a transported byte is corrupt: stdout is
empty, no active profile is published, and the Ticket 005 integrity code is
preserved.

```bash run id=local-assets-corruption exit=1 stream=stderr
cp -a ../target/spec/local-assets/transport ../target/spec/local-assets/corrupt-transport
printf X | dd of=../target/spec/local-assets/corrupt-transport/payload.pgi.zst.part0000 bs=1 seek=20 count=1 conv=notrunc status=none
corrupt_data=$(cd .. && pwd)/target/spec/local-assets/corrupt-data
set +e
pangopup assets install --transport ../target/spec/local-assets/corrupt-transport --data-dir "$corrupt_data" >../target/spec/local-assets/corrupt.stdout 2>../target/spec/local-assets/corrupt.stderr
status=$?
set -e
test "$status" -eq 1
test ! -s ../target/spec/local-assets/corrupt.stdout
test ! -e "$corrupt_data/active.json"
cat ../target/spec/local-assets/corrupt.stderr >&2
exit "$status"
```

```text expect=local-assets-corruption contains
{"status":"error","code":"TRANSPORT_HASH_MISMATCH"
```
