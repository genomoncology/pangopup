# README first-use contract

The README stays a compact user guide. These checks inspect text only; they do
not use the network, build Docker, synchronize assets, or remove files.

```bash
test "$(wc -l < ../README.md)" -le 350
test "$(wc -w < ../README.md)" -le 2200
printf 'README size is bounded\n' | mustmatch like 'README size is bounded'
```

The immutable versioned delivery commands are explicit and the tagged guide is
free of candidate or pending-publication language:

```bash
rg -F 'immutable **v0.3.0** application release' ../README.md >/dev/null
! rg -i 'release candidate|publication is still pending|after publication' ../README.md
rg -F 'raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh' ../README.md >/dev/null
rg -F 'ghcr.io/genomoncology/pangopup:0.3.0' ../README.md >/dev/null
printf 'immutable delivery is explicit\n' | mustmatch like 'immutable delivery is explicit'
```

The top of the guide explains the scientific result before a bounded,
CLI-only first-use path:

```bash
what=$(awk '/^## What it predicts$/{on=1; next} /^## Quick start: CLI$/{on=0} on' ../README.md)
quick=$(awk '/^## Quick start: CLI$/{on=1; next} /^## Storage and memory$/{on=0} on' ../README.md)
test "$(rg -n '^## (Introduction|What it predicts|Quick start: CLI|Storage and memory)$' ../README.md | head -4 | cut -d: -f2-)" = "$(printf '%s\n' '## Introduction' '## What it predicts' '## Quick start: CLI' '## Storage and memory')"
for text in 'splice-site usage' 'gain (increase)' 'signed loss (decrease)' '50 bases on either side of the variant' "deletion's allele span can extend the reported positive offset" 'Overlapping genes' 'separate records' 'genomic-coordinate offset' 'higher genomic coordinate' 'minus-strand gene' 'not a transcript-oriented distance' 'pathogenicity classification' 'clinical diagnosis' 'exact RNA transcript or protein consequence' 'does not expose tissue-specific scores' 'Tony Zeng' 'Yang I. Li' 'https://doi.org/10.1186/s13059-022-02664-4'; do printf '%s' "$what" | rg -F "$text" >/dev/null; done
for text in 'Linux x86-64/amd64 with GLIBC 2.39 or newer' '2.44 GiB' '14.76 GiB' '25 GB' 'raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh' 'bash -s -- --version 0.3.0' 'export PATH="$HOME/.local/bin:$PATH"' 'pangopup sync --progress' 'pangopup status' 'pangopup lookup --variant GRCh38:chr12:6801301:G:A' 'pangopup lookup --variant GRCh38:chr12:6801303:G:GA' 'JSON Lines' '`provenance.kind`' '`precomputed` or `model`' 'network-free after' '[Install on Linux](#install-on-linux)' '[Score from the CLI](#score-from-the-cli)'; do printf '%s' "$quick" | rg -F "$text" >/dev/null; done
! printf '%s' "$quick" | rg -i '(^|[[:space:]])docker([[:space:]]|$)|pangopup serve|/v1/(score|status)|127\.0\.0\.1|git clone|cargo build|PANGOPUP_(DATA|CACHE)|--model-only|--format table|uninstall' >/dev/null
printf 'prediction and CLI quick start are bounded\n' | mustmatch like 'prediction and CLI quick start are bounded'
```

The exact authenticated storage evidence and mmap explanation remain visible:

```bash
rg -F '1,931,694,270 bytes' ../README.md >/dev/null
rg -F '15,033,158,255 bytes' ../README.md >/dev/null
rg -F '691,874,664 bytes' ../README.md >/dev/null
rg -F '812,662,222 bytes' ../README.md >/dev/null
rg -F '2,623,568,934 bytes' ../README.md >/dev/null
rg -F '15,845,820,477 bytes' ../README.md >/dev/null
rg -F '25 GB free' ../README.md >/dev/null
rg -F 'fixed-width records' ../README.md >/dev/null
rg -F 'reserves virtual address space' ../README.md >/dev/null
rg -F 'not 15 GB of Rust heap' ../README.md >/dev/null
printf 'storage and mmap guidance are present\n' | mustmatch like 'storage and mmap guidance are present'
```

Measured resource claims retain their identity and limitations:

```bash
rg -F 'five-round, warm-page-cache observation' ../README.md >/dev/null
rg -F 'AMD Ryzen 7 5825U' ../README.md >/dev/null
rg -F '12.0 / 12.3 MiB peak RSS' ../README.md >/dev/null
rg -F '102.3 / 102.5 MiB PSS' ../README.md >/dev/null
rg -F '105.8 / 106.0 MiB RSS' ../README.md >/dev/null
rg -F '137.0 / 137.3 MiB high-water RSS' ../README.md >/dev/null
rg -F '0.8 / 0.5 / 1.1 ms median' ../README.md >/dev/null
rg -F '4.30 / 5.70 seconds' ../README.md >/dev/null
rg -F '0.7 / 0.9 ms' ../README.md >/dev/null
rg -F 'observations, not universal requirements or cold-cache guarantees' ../README.md >/dev/null
rg -F 'planning/artifacts/053-current-runtime-resources.md' ../README.md >/dev/null
printf 'measurement boundary is present\n' | mustmatch like 'measurement boundary is present'
```

The direct first-use path and variant grammar remain discoverable:

```bash
rg -F 'Linux x86-64/amd64 with GLIBC 2.39 or newer' ../README.md >/dev/null
rg -F 'Current source requires Git, Rust 1.93' ../README.md >/dev/null || rg -F 'use Git, Rust 1.93' ../README.md >/dev/null
rg -F 'git rev-parse HEAD' ../README.md >/dev/null
rg -F 'pangopup sync --progress' ../README.md >/dev/null
rg -F 'pangopup status' ../README.md >/dev/null
rg -F 'pangopup sync --offline' ../README.md >/dev/null
rg -F 'GRCh38:CONTIG:POS:REF:ALT' ../README.md >/dev/null
rg -F 'pangopup lookup --variant GRCh38:chr12:6801301:G:A' ../README.md >/dev/null
rg -F 'pangopup lookup --model-only' ../README.md >/dev/null
printf 'first-use commands are present\n' | mustmatch like 'first-use commands are present'
```

XDG paths and the two distinct cache controls remain exact:

```bash
rg -F '~/.local/share/pangopup' ../README.md >/dev/null
rg -F '~/.cache/pangopup' ../README.md >/dev/null
rg -F 'model-results.sqlite3' ../README.md >/dev/null
rg -F '`PANGOPUP_CACHE_DIR` relocates only resumable transport downloads; it does not' ../README.md >/dev/null
rg -F 'The model-result path precedence is `--model-cache`, then' ../README.md >/dev/null
rg -F '`PANGOPUP_MODEL_CACHE`, then `$XDG_CACHE_HOME/pangopup/model-results.sqlite3`' ../README.md >/dev/null
printf 'XDG and cache paths are present\n' | mustmatch like 'XDG and cache paths are present'
```

HTTP examples include all routes, request keys, and security limits:

```bash
rg -F 'pangopup serve --listen 127.0.0.1:8080' ../README.md >/dev/null
for route in /livez /readyz /v1/status /v1/score; do rg -F "$route" ../README.md >/dev/null; done
rg -F '"model_only":true' ../README.md >/dev/null
rg -F 'this abridged model output' ../README.md >/dev/null
rg -F '`precomputed` means' ../README.md >/dev/null
rg -F '`model` means' ../README.md >/dev/null
rg -F 'no built-in authentication or TLS' ../README.md >/dev/null
rg -F 'authenticated TLS reverse proxy' ../README.md >/dev/null
printf 'HTTP and provenance are present\n' | mustmatch like 'HTTP and provenance are present'
```

Docker usage preserves architecture, immutable identity, and volume lifecycle:

```bash
rg -F 'docker volume create pangopup-data' ../README.md >/dev/null
rg -F 'docker volume create pangopup-cache' ../README.md >/dev/null
rg -F 'pangopup-data:/var/lib/pangopup:ro' ../README.md >/dev/null
rg -F 'pangopup-cache:/var/cache/pangopup' ../README.md >/dev/null
rg -F '"$PANGOPUP_IMAGE" status' ../README.md >/dev/null
rg -F '"$PANGOPUP_IMAGE" lookup --variant GRCh38:chr12:6801301:G:A' ../README.md >/dev/null
rg -F 'From another terminal, exercise the running container:' ../README.md >/dev/null
rg -F 'docker buildx imagetools inspect "$PANGOPUP_IMAGE"' ../README.md >/dev/null
rg -F 'ghcr.io/genomoncology/pangopup@sha256:<INDEX_DIGEST>' ../README.md >/dev/null
rg -F 'native Linux ARM64 code' ../README.md >/dev/null
rg -F 'not MPS or Metal' ../README.md >/dev/null
rg -F 'Unknown CPU vendor. cpuinfo_vendor value: 0' ../README.md >/dev/null
rg -F 'Apple-aware `cpuinfo`' ../README.md >/dev/null
printf 'Docker and Apple boundaries are present\n' | mustmatch like 'Docker and Apple boundaries are present'
```

Safe update, uninstall, and host-side Docker removal stay visible:

```bash
rg -F '## Update and uninstall' ../README.md >/dev/null
rg -F 'raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh' ../README.md >/dev/null
rg -F 'docker pull "$PANGOPUP_IMAGE"' ../README.md >/dev/null
rg -F 'docker stop pangopup' ../README.md >/dev/null
rg -F 'while reusing both named volumes' ../README.md >/dev/null
rg -F 'pangopup uninstall --full' ../README.md >/dev/null
rg -F 'pangopup uninstall --yes' ../README.md >/dev/null
rg -F 'pangopup uninstall --full --yes' ../README.md >/dev/null
rg -F '`PANGOPUP_MODEL_CACHE` outside the managed cache root is not discoverable' ../README.md >/dev/null
rg -F 'docker image rm ghcr.io/genomoncology/pangopup:0.3.0' ../README.md >/dev/null
rg -F 'docker volume rm pangopup-cache' ../README.md >/dev/null
rg -F 'docker volume rm pangopup-data' ../README.md >/dev/null
printf 'update and removal guidance is present\n' | mustmatch like 'update and removal guidance is present'
```

Attribution and maintainer links remain present without engineering history:

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
