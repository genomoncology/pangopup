# Pinned remote SNV assets

The public binary exposes one explicit sync command. This executable spec uses
offline mode and isolated empty directories, so it never contacts GitHub or
downloads a production asset.

```bash run id=remote-assets-offline-missing exit=1 stream=stderr
chmod -R u+w ../target/spec/remote-assets 2>/dev/null || true
rm -rf ../target/spec/remote-assets
mkdir -p ../target/spec/remote-assets
data=$(cd .. && pwd)/target/spec/remote-assets/data
cache=$(cd .. && pwd)/target/spec/remote-assets/cache
pangopup sync --offline --data-dir "$data" --cache-dir "$cache"
```

```text expect=remote-assets-offline-missing contains
{"status":"error","code":"ASSET_SYNC_INCOMPLETE","message":"asset synchronization did not complete","details":{"snv":{"status":"error","code":"ASSETS_MISSING","message":"profile snv-grch38-v1 is incomplete:
```

Every present cache-path input is validated even when a higher-precedence input
would otherwise win. Relative and empty configuration never falls through.

```bash run id=remote-assets-relative-cache exit=2 stream=stderr
PANGOPUP_CACHE_DIR=relative XDG_CACHE_HOME=/tmp/pangopup-unused pangopup sync --offline --data-dir /tmp/pangopup-unused-data
```

```text expect=remote-assets-relative-cache contains
{"status":"error","code":"PATH_INVALID","message":"PANGOPUP_CACHE_DIR must be a nonempty absolute UTF-8 path"
```

The offline flag is a flag, not a value-bearing option, and may appear once.

```bash run id=remote-assets-duplicate-offline exit=2 stream=stderr
pangopup sync --offline --offline
```

```text expect=remote-assets-duplicate-offline contains
{"status":"error","code":"CLI_USAGE","message":"--offline may be supplied once"
```
