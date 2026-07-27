# Local runtime-profile installation

Runtime status is offline and missing state is a normal compact result.

```bash
rm -rf ../target/spec/runtime-install
data=$(cd .. && pwd)/target/spec/runtime-install/data
pangopup assets runtime status --data-dir "$data" | sed "s|$data|<data>|" | mustmatch like '{"status":"missing","data_dir":"<data>"}'
```

The nested grammar is closed. A complete install request requires all four
inputs; status accepts only the data-root override.

```bash run id=runtime-install-missing-inputs exit=2 stream=stderr
pangopup assets runtime install --profile /tmp/profile.json
```

```text expect=runtime-install-missing-inputs exact
{"status":"error","code":"CLI_USAGE","message":"assets runtime install requires --model-bundle","details":null}
```

```bash run id=runtime-status-extra-option exit=2 stream=stderr
pangopup assets runtime status --mask /tmp/domains.pgm
```

```text expect=runtime-status-extra-option exact
{"status":"error","code":"CLI_USAGE","message":"unknown assets runtime option --mask","details":null}
```

A valid production profile cannot install without an already certified active
SNV object. Later source paths are not opened and no runtime pointer or
component is created.

```bash run id=runtime-install-needs-snv exit=1 stream=stderr
rm -rf ../target/spec/runtime-install/no-snv
data=$(cd .. && pwd)/target/spec/runtime-install/no-snv
pangopup assets runtime install --profile ../planning/artifacts/024-four-asset-runtime-profile.json --model-bundle /secret/not-opened --reference-bundle /secret/not-opened --mask /secret/not-opened --data-dir "$data"
```

```text expect=runtime-install-needs-snv exact
{"status":"error","code":"ASSETS_MISSING","message":"an active SNV bundle is required","details":null}
```

```bash
data=$(cd .. && pwd)/target/spec/runtime-install/no-snv
test ! -e "$data/runtime/active.json"
test ! -e "$data/runtime/components"
printf 'no partial runtime output\n' | mustmatch like 'no partial runtime output'
```

Present malformed runtime state is an error rather than `missing`, and the
diagnostic does not expose file contents.

```bash run id=runtime-status-malformed exit=1 stream=stderr
data=$(cd .. && pwd)/target/spec/runtime-install/malformed
rm -rf "$data"
install -d -m 700 "$data/runtime"
printf '{}' > "$data/runtime/active.json"
chmod 600 "$data/runtime/active.json"
pangopup assets runtime status --data-dir "$data"
```

```text expect=runtime-status-malformed exact
{"status":"error","code":"PROFILE_CORRUPT","message":"installed JSON is invalid","details":null}
```
