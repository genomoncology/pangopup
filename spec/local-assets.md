# Local asset installation

The runtime installs an already available Ticket 005 transport into an
isolated absolute data root. The spec uses only the checked-in miniature build
fixture and never reads a production asset.

```bash
chmod -R u+w ../target/spec/local-assets 2>/dev/null || true
rm -rf ../target/spec/local-assets
mkdir -p ../target/spec/local-assets
cp -R ../tests/fixtures/snv-regression/bundle ../target/spec/local-assets/bundle
pangopup-build transport pack --bundle ../target/spec/local-assets/bundle --output ../target/spec/local-assets/transport >/dev/null
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup status --data-dir "$data" | sed "s|$data|<data>|" | mustmatch like '{"status":"missing","data_dir":"<data>","syncing":false,"installing":false,"snv":{"status":"missing"},"runtime":{"status":"missing"}}'
```

Install publishes one immutable bundle and an active profile. Successful
stdout is one compact object whose path names the three-member bundle itself.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup assets install --transport ../target/spec/local-assets/transport --data-dir "$data" | sed -E "s|$data|<data>|; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"installed","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle"}'
pangopup status --data-dir "$data" | sed -E "s|$data|<data>|g; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"partial","data_dir":"<data>","syncing":false,"installing":false,"snv":{"status":"ready","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle"},"runtime":{"status":"missing"}}'
LC_ALL=C ls -laR "$data" | sed '/ \.\.$/d' > ../target/spec/local-assets/status-before
pangopup status --data-dir "$data" >/dev/null
LC_ALL=C ls -laR "$data" | sed '/ \.\.$/d' > ../target/spec/local-assets/status-after
cmp ../target/spec/local-assets/status-before ../target/spec/local-assets/status-after
set -- "$data"/bundles/*
test "$#" -eq 1
object=$1
test "$(LC_ALL=C ls -ld "$data" | cut -c 1-10)" = drwx------
test "$(LC_ALL=C ls -l "$data/active.json" | cut -c 1-10)" = -rw-------
test "$(LC_ALL=C ls -ld "$object" | cut -c 1-10)" = dr-xr-xr-x
test "$(LC_ALL=C ls -ld "$object/bundle" | cut -c 1-10)" = dr-xr-xr-x
test "$(LC_ALL=C ls -l "$object/bundle/scores.pgi" | cut -c 1-10)" = -r--r--r--
printf 'private atomic installation\n' | mustmatch like 'private atomic installation'
```

Status holds a read-only shared observation guard across the SNV and runtime
pointers. It does not create or change installed entries. Installers retain the
exclusive guard; under contention status returns promptly with
`installing: true` and best-effort component observations.

Implicit lookup discovers the active bundle and preserves the exact existing
lookup bytes. The explicit override remains compatible and cannot be combined
with `--data-dir`.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup lookup --bundle ../target/spec/local-assets/bundle --variant GRCh38:chr12:6801301:G:A > ../target/spec/local-assets/explicit.jsonl
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A > ../target/spec/local-assets/implicit.jsonl
cmp ../target/spec/local-assets/explicit.jsonl ../target/spec/local-assets/implicit.jsonl
printf 'byte-identical active lookup\n' | mustmatch like 'byte-identical active lookup'
```

```bash run id=local-assets-mutual-exclusion exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/local-assets/bundle --data-dir /tmp/pangopup-unused --variant GRCh38:chr1:1:A:C
```

```text expect=local-assets-mutual-exclusion contains
{"status":"error","code":"CLI_USAGE"
```

Reuse validates installed metadata and cheap-open structure without reading a
transport payload or hashing the installed score payload.

```bash
data=$(cd .. && pwd)/target/spec/local-assets/data
pangopup assets install --transport ../target/spec/local-assets/transport --data-dir "$data" | sed -E "s|$data|<data>|; s/sha256:[0-9a-f]{64}/sha256:<digest>/g; s|/bundles/[0-9a-f]{64}/bundle|/bundles/<digest>/bundle|" | mustmatch like '{"status":"reused","bundle_id":"sha256:<digest>","transport_id":"sha256:<digest>","path":"<data>/bundles/<digest>/bundle"}'
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
PANGOPUP_DATA_DIR=relative XDG_DATA_HOME=/tmp/pangopup-unused pangopup status
```

```text expect=local-assets-relative-path contains
{"status":"error","code":"PATH_INVALID"
```

Install remains transactional when a transported byte is corrupt: stdout is
empty, no active profile is published, and the Ticket 005 integrity code is
preserved.

```bash run id=local-assets-corruption exit=1 stream=stderr
cp -R ../target/spec/local-assets/transport ../target/spec/local-assets/corrupt-transport
chmod -R u+w ../target/spec/local-assets/corrupt-transport
printf X | dd of=../target/spec/local-assets/corrupt-transport/payload.pgi.zst.part0000 bs=1 seek=20 count=1 conv=notrunc 2>/dev/null
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
