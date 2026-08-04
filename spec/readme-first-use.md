# README first-use contract

The README stays a compact user guide. These checks inspect its text only; they
do not use the network, build Docker, synchronize assets, or execute removal
commands.

```bash
test "$(wc -l < ../README.md)" -le 450
test "$(wc -w < ../README.md)" -le 3000
printf 'README size is bounded\n' | mustmatch like 'README size is bounded'
```

It presents v0.2.0 as the ordinary release while retaining the older immutable
v0.1.0 release:

```bash
rg -F '**`v0.2.0` is the ordinary Linux release.**' ../README.md >/dev/null
rg -F 'Immutable `v0.1.0` remains' ../README.md >/dev/null
rg -F 'raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh' ../README.md >/dev/null
rg -F '`ghcr.io/genomoncology/pangopup:0.2.0`' ../README.md >/dev/null
printf 'delivery tracks are explicit\n' | mustmatch like 'delivery tracks are explicit'
```

The copy/paste first-use path and variant grammar remain discoverable:

```bash
rg -F 'raw.githubusercontent.com/genomoncology/pangopup/main/install.sh' ../README.md >/dev/null
rg -F 'pangopup sync --progress' ../README.md >/dev/null
rg -F 'pangopup status' ../README.md >/dev/null
rg -F 'pangopup lookup --variant GRCh38:chr12:6801301:G:A' ../README.md >/dev/null
rg -F 'pangopup lookup --model-only' ../README.md >/dev/null
rg -F 'pangopup serve --listen 127.0.0.1:8080' ../README.md >/dev/null
rg -F 'GRCh38:CONTIG:POS:REF:ALT' ../README.md >/dev/null
rg -F 'Current source requires Git, Rust 1.93' ../README.md >/dev/null
rg -F 'git rev-parse HEAD' ../README.md >/dev/null
printf 'first-use commands are present\n' | mustmatch like 'first-use commands are present'
```

The HTTP routes, complete JSON request keys, and provenance explanation stay
present:

```bash
for route in /livez /readyz /v1/status /v1/score; do rg -F "$route" ../README.md >/dev/null; done
rg -F '"model_only":true' ../README.md >/dev/null
rg -F '"provenance":{"kind":"model"' ../README.md >/dev/null
rg -F '`precomputed` means' ../README.md >/dev/null
rg -F '`model` means' ../README.md >/dev/null
printf 'HTTP and provenance are present\n' | mustmatch like 'HTTP and provenance are present'
```

Data, cache, disk, and offline behavior remain explicit:

```bash
rg -F '~/.local/share/pangopup' ../README.md >/dev/null
rg -F '~/.cache/pangopup' ../README.md >/dev/null
rg -F 'model-results.sqlite3' ../README.md >/dev/null
rg -F '`PANGOPUP_CACHE_DIR` relocates only resumable transport downloads; it does not relocate SQLite.' ../README.md >/dev/null
rg -F 'The model-result path precedence is `--model-cache`, then `PANGOPUP_MODEL_CACHE`, then' ../README.md >/dev/null
rg -F '`$XDG_CACHE_HOME/pangopup/model-results.sqlite3`, then `$HOME/.cache/pangopup/model-results.sqlite3`.' ../README.md >/dev/null
rg -F '25 GB free' ../README.md >/dev/null
rg -F '14.8 GB' ../README.md >/dev/null
rg -F '2.4 GB' ../README.md >/dev/null
rg -F 'pangopup sync --offline' ../README.md >/dev/null
printf 'storage guidance is present\n' | mustmatch like 'storage guidance is present'
```

Docker usage preserves the durable/disposable volume distinction and the
qualified read-only data mount:

```bash
rg -F 'docker volume create pangopup-data' ../README.md >/dev/null
rg -F 'docker volume create pangopup-cache' ../README.md >/dev/null
rg -F 'docker pull "$PANGOPUP_IMAGE"' ../README.md >/dev/null
rg -F 'docker buildx imagetools inspect "$PANGOPUP_IMAGE"' ../README.md >/dev/null
rg -F 'ghcr.io/genomoncology/pangopup@sha256:<INDEX_DIGEST>' ../README.md >/dev/null
rg -F 'pangopup-data:/var/lib/pangopup:ro' ../README.md >/dev/null
rg -F 'pangopup-cache:/var/cache/pangopup' ../README.md >/dev/null
rg -F 'http://127.0.0.1:8080/v1/score' ../README.md >/dev/null
rg -F 'docker image rm ghcr.io/genomoncology/pangopup:0.2.0' ../README.md >/dev/null
rg -F 'docker volume rm pangopup-cache' ../README.md >/dev/null
rg -F 'docker volume rm pangopup-data' ../README.md >/dev/null
printf 'Docker volume lifecycle is present\n' | mustmatch like 'Docker volume lifecycle is present'
```

Platform and service-security boundaries cannot silently disappear:

```bash
rg -F 'Linux x86-64/amd64 with GLIBC 2.39 or newer' ../README.md >/dev/null
rg -F 'native Linux ARM64 code' ../README.md >/dev/null
rg -F 'not MPS or Metal' ../README.md >/dev/null
rg -F 'Unknown CPU vendor. cpuinfo_vendor value: 0' ../README.md >/dev/null
rg -F 'wait for an upstream ONNX' ../README.md >/dev/null
rg -F 'Runtime release containing Apple-aware `cpuinfo`' ../README.md >/dev/null
rg -F 'no built-in authentication or TLS' ../README.md >/dev/null
rg -F 'authenticated TLS reverse proxy' ../README.md >/dev/null
printf 'platform and security boundaries are present\n' | mustmatch like 'platform and security boundaries are present'
```

Update and safe CLI uninstall guidance remains visible;
the spec only matches these commands and never executes them:

```bash
rg -F '## Update' ../README.md >/dev/null
rg -F '## Uninstall' ../README.md >/dev/null
rg -F 'pangopup uninstall' ../README.md >/dev/null
rg -F 'pangopup uninstall --full' ../README.md >/dev/null
rg -F 'pangopup uninstall --yes' ../README.md >/dev/null
rg -F 'pangopup uninstall --full --yes' ../README.md >/dev/null
rg -F '`PANGOPUP_MODEL_CACHE` outside that root is not discoverable and is not' ../README.md >/dev/null
printf 'update and removal guidance is present\n' | mustmatch like 'update and removal guidance is present'
```

Licenses, upstream attribution, and the detailed maintainer references remain
linked rather than copied into the first-use narrative:

```bash
rg -F 'GPL-3.0-only' ../README.md >/dev/null
rg -F '10.5281/zenodo.15649338' ../README.md >/dev/null
rg -F 'CC BY 4.0' ../README.md >/dev/null
rg -F 'GENCODE v38' ../README.md >/dev/null
rg -F '[`NOTICE`](NOTICE)' ../README.md >/dev/null
rg -F '[Architecture overview](architecture/README.md)' ../README.md >/dev/null
rg -F '[Current project frontier](planning/frontier.md)' ../README.md >/dev/null
rg -F 'pangopup-build --help' ../README.md >/dev/null
printf 'attribution and maintainer links are present\n' | mustmatch like 'attribution and maintainer links are present'
```
