# PangoPup

PangoPup is a fast, standalone, open-source service for
[Pangolin](https://github.com/tkzeng/Pangolin)-compatible splice scores on
GRCh38 variants. It is written in Rust and has two scoring paths:

- For a single-nucleotide variant (SNV), it first looks up the published
  precomputed Pangolin score in a memory-mapped index.
- For an SNV absent from that index, or for a supported insertion, deletion,
  or multi-nucleotide variant, it runs the Pangolin model on the CPU.

The lookup path avoids loading the 15 GB index into application memory; the
operating system maps only the pages a request touches. Model results are
stored in a persistent SQLite cache, so repeating the same modeled request is
normally a cache read.

PangoPup returns splice gain and loss scores, their relative positions, gene
records, and the provenance of the result. It does not interpret clinical
significance, parse HGVS, project variants to transcripts or proteins, or
provide gene and disease knowledge.

## Choose a version

**`v0.2.0` is the ordinary Linux release.** It includes asset sync/status,
lookup-first and explicit model-only CLI scoring, resilient download progress,
focused help, and the foreground HTTP service. Immutable `v0.1.0` remains
available as the older CLI-only release.

The matching thin AMD64/ARM64 container is
`ghcr.io/genomoncology/pangopup:0.2.0`. It contains the executable and notices,
not the separately synchronized scoring assets.

## Install the Linux executable

Prerequisites:

- Linux x86-64/amd64 with GLIBC 2.39 or newer;
- Bash;
- `curl` or `wget`;
- `sha256sum`, `shasum`, or `openssl`.

Install the latest published executable for your user:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/main/install.sh | bash
export PATH="$HOME/.local/bin:$PATH"
```

To pin the immutable `v0.2.0` installer and executable:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh \
  | bash -s -- --version 0.2.0
```

The installer verifies the executable's SHA-256 checksum, smoke-tests it, and
atomically writes `${PANGOPUP_INSTALL_DIR:-$HOME/.local/bin}/pangopup`. It does
not use `sudo`, edit `PATH`, or download the scoring assets.

## Build from source

Current source requires Git, Rust 1.93, and a C/C++ build toolchain:

```bash
git clone https://github.com/genomoncology/pangopup.git
cd pangopup
git rev-parse HEAD                              # record this 40-character ID
git checkout --detach HEAD
cargo build --locked --release --package pangopup-cli
install -Dm755 target/release/pangopup "$HOME/.local/bin/pangopup"
export PATH="$HOME/.local/bin:$PATH"
```

For reproducible use, record the 40-character commit printed by
`git rev-parse HEAD` rather than treating a changing branch name as a version.

## Download the scoring assets

Allow at least **25 GB free** for the first synchronization. Retained Apple
Silicon Docker observations after a completed sync were about 14.8 GB of
installed data and 2.4 GB of cache; provisioning needs additional working
space while compressed downloads are verified and installed.

```bash
pangopup sync --progress
pangopup status
```

Synchronization downloads two pinned, immutable GitHub releases: the
precomputed SNV index and the model/reference/mask runtime. It verifies sizes
and SHA-256 checksums, resumes safe partial downloads, and never selects an
asset release named `latest`. It does not download or redistribute the raw
Zenodo, NCBI, or GENCODE source inputs.

The default Linux locations follow XDG:

| Purpose | Default | Override |
|---|---|---|
| Installed, reusable assets | `~/.local/share/pangopup` | `PANGOPUP_DATA_DIR` or `XDG_DATA_HOME` |
| Resumable transport downloads | `~/.cache/pangopup` | `PANGOPUP_CACHE_DIR` or `XDG_CACHE_HOME` |
| SQLite model-result cache | `~/.cache/pangopup/model-results.sqlite3` | `--model-cache`, `PANGOPUP_MODEL_CACHE`, or XDG defaults |

The two cache uses share a default directory but are resolved independently.
`PANGOPUP_CACHE_DIR` relocates only resumable transport downloads; it does not relocate SQLite.
The model-result path precedence is `--model-cache`, then `PANGOPUP_MODEL_CACHE`, then
`$XDG_CACHE_HOME/pangopup/model-results.sqlite3`, then `$HOME/.cache/pangopup/model-results.sqlite3`.

Use absolute paths for overrides. Once installed, scoring is network-free.
Confirm that all assets can be reused offline with:

```bash
pangopup sync --offline
pangopup status
```

## Score variants from the CLI

Variants use the literal form `GRCh38:CONTIG:POS:REF:ALT`, with a 1-based
position. Contigs may be `1` through `22`, `X`, `Y`, `M`, their `chr` forms,
or the exact RefSeq accessions in the installed manifest.

Score one SNV:

```bash
pangopup lookup --variant GRCh38:chr12:6801301:G:A
```

Score a batch and render tab-separated output:

```bash
pangopup lookup \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr12:6801303:G:GA \
  --format table
```

JSON Lines is the default. Add `--gene ENSG...` to retain only one stable
Ensembl gene ID. Automatic routing uses the precomputed index when it contains
the SNV and otherwise invokes the model.

Bypass the SNV index deliberately:

```bash
pangopup lookup --model-only \
  --variant GRCh38:chr12:6801301:G:A
```

`--model-only` is useful for comparing the current model with a precomputed
score.

### Reading a result

A successful JSONL record looks like this (long asset hashes shortened here):

```json
{"assembly":"GRCh38","contig":"chr12","position":6801301,"ref":"G","alt":"A","status":"found","records":[{"gene":"ENSG00000010610","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:...","source_doi":"10.5281/zenodo.15649338","masked":true,"window":50}}
```

`provenance.kind` explains which path produced the answer:

- `precomputed` means the exact stored Zenodo-derived SNV score was returned.
- `model` means PangoPup ran the Pangolin-compatible model or reused its exact
  SQLite-cached result. Model provenance identifies the model, reference,
  mask, scoring semantics, and CPU policy.

Scores are strings so values retain their defined decimal representation.
Loss scores are signed. A result can contain multiple gene records where genes
overlap.

## Run the HTTP service

The service runs in the foreground and listens on loopback by default:

```bash
pangopup serve --listen 127.0.0.1:8080
```

Check health and readiness:

```bash
curl -fsS http://127.0.0.1:8080/livez
curl -fsS http://127.0.0.1:8080/readyz
curl -fsS http://127.0.0.1:8080/v1/status
```

Score one or more variants with automatic routing:

```bash
curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr12:6801303:G:GA"]}'
```

Force model scoring, with an optional stable Ensembl gene filter:

```bash
curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801303:G:GA"],"gene":"ENSG00000010610","model_only":true}'
```

The complete response envelope contains the same records as the CLI:

```json
{"results":[{"assembly":"GRCh38","contig":"chr12","position":6801303,"ref":"G","alt":"GA","status":"found","records":[{"gene":"ENSG00000010610.10","gain_score":"0.00","gain_position":0,"loss_score":"0.00","loss_position":-50,"warnings":[]}],"source_reference_ambiguities":[],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","model_bundle_id":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","model_profile":"pangolin-1.0.2-5cf94b8-onnx-cpu-v1","effective_cpu_policy":"sequential:1/1","reference_bundle_id":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_profile":"refseq-grch38p14-primary-v1","reference_sequence_set_sha256":"sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4","mask_bytes":6703320,"mask_sha256":"sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702","masked":true,"window":50}}]}
```

`POST /v1/score` accepts at most 100 variants, of which at most 10 may require
uncached model work. The service uses a bounded model queue and returns HTTP
429 when that queue is full. Clients may wait normally for the HTTP response;
there is no polling job API.

The service has **no built-in authentication or TLS**. Keep the default
loopback listener for local use. Before binding to a non-loopback address, put
PangoPup behind an authenticated TLS reverse proxy and apply your own access
and request limits.

## Docker on AMD64 and Apple Silicon

Pull the versioned image. Docker selects the native Linux AMD64 or ARM64 child:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.2.0
docker pull "$PANGOPUP_IMAGE"
docker volume create pangopup-data
docker volume create pangopup-cache
```

Download assets once into the named volumes:

```bash
docker run --rm \
  -v pangopup-data:/var/lib/pangopup \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" sync --progress
```

Check status without giving the process write access to installed assets:

```bash
docker run --rm --network none \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" status
```

Run a CLI lookup without networking:

```bash
docker run --rm --network none \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" lookup --variant GRCh38:chr12:6801301:G:A
```

Run the foreground service:

```bash
docker run --rm --name pangopup -p 127.0.0.1:8080:8080 \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE"
```

From another terminal, check the running service:

```bash
curl -fsS http://127.0.0.1:8080/livez
curl -fsS -H 'content-type: application/json' \
  -d '{"variants":["GRCh38:chr12:6801301:G:A"],"model_only":false}' \
  http://127.0.0.1:8080/v1/score
```

The image runs as non-root UID/GID 65532 and contains no scoring assets. Named
volumes survive container replacement. The data volume is durable and costly
to recreate; the cache volume contains resumable downloads and disposable
SQLite model results.

On Apple Silicon, Docker builds and runs native Linux ARM64 code. Model
inference uses the ARM64 **CPU**, not MPS or Metal. ONNX Runtime may print this
harmless warning:

```text
onnxruntime cpuid_info warning: Unknown CPU vendor. cpuinfo_vendor value: 0
```

Matched testing traced it to ONNX Runtime's older CPU-identification data, not
to emulation or a scoring difference. PangoPup will wait for an upstream ONNX
Runtime release containing Apple-aware `cpuinfo` rather than maintain a custom
runtime solely to hide the message.

Docker, systemd, Kubernetes, or another process manager owns start, stop, and
restart. PangoPup intentionally has no daemon-management commands.

For an immutable deployment, inspect the versioned OCI index and copy its
top-level `Digest` value:

```bash
docker buildx imagetools inspect "$PANGOPUP_IMAGE"
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup@sha256:<INDEX_DIGEST>
docker pull "$PANGOPUP_IMAGE"
```

The version tag is convenient for people; the digest identifies exact bytes.
To build instead, clone and check out the intended commit, then run:

```bash
docker build \
  --build-arg PANGOPUP_REVISION="$(git rev-parse HEAD)" \
  --build-arg PANGOPUP_VERSION="$(git describe --always --dirty)" \
  -t pangopup:local .
```

## Update

Updating the executable does not require downloading the assets again.

For a published Linux release, rerun the installer (optionally with
`--version MAJOR.MINOR.PATCH`), then verify and reuse the installed assets:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/main/install.sh | bash
pangopup sync --offline
pangopup status
```

For Docker, pull the intended new version or digest, stop and replace the
foreground container, and reuse the same two named volumes:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.2.0
docker pull "$PANGOPUP_IMAGE"
docker stop pangopup
docker run --rm --name pangopup -p 127.0.0.1:8080:8080 \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE"
```

Run `sync` if a later image declares a different pinned asset profile;
otherwise `sync --offline` confirms reuse without downloading again.

## Uninstall

For a direct Linux installation, PangoPup checks and displays the executable,
data, and cache paths before it removes anything:

```bash
pangopup uninstall
```

Choose code only, code plus all managed data, or cancel. Code-only removal
preserves the large installed scoring assets, resumable downloads, and SQLite
model-result cache for a later reinstall. `--full` selects code plus all
managed data but still asks for confirmation:

```bash
pangopup uninstall --full
```

For scripts, `--yes` skips the prompt. It removes code only unless combined
with `--full`:

```bash
pangopup uninstall --yes
pangopup uninstall --full --yes
```

Full removal includes installed assets (about 14.8 GB in the retained Mac
run), resumable downloads, and the default SQLite cache inside the resolved
cache root (about 2.4 GB after first sync). A separately configured
`PANGOPUP_MODEL_CACHE` outside that root is not discoverable and is not
removed. Stop a foreground PangoPup service first; PangoPup deliberately has
no process registry or supervisor. The command refuses unsafe, aliased,
foreign-owned, busy, or unexpected paths and removes the executable last.

Containers cannot remove host images or volumes. For Docker, remove those on
the host:

```bash
docker image rm ghcr.io/genomoncology/pangopup:0.2.0
docker volume rm pangopup-cache       # removes downloads and model-result cache
docker volume rm pangopup-data        # removes the large installed assets
```

The command resolves data and cache through `PANGOPUP_DATA_DIR`,
`PANGOPUP_CACHE_DIR`, XDG, and `HOME`. The current executable is resolved by
the operating system; `PANGOPUP_INSTALL_DIR` is an installer setting, not an
uninstall path override.

## Source, licenses, and attribution

PangoPup source is licensed under
[GPL-3.0-only](https://github.com/genomoncology/pangopup/blob/main/LICENSE).
Its converted runtime derives from Pangolin 1.0.2 by Tony Zeng and retains the
Pangolin GPL source, model, and modification material in the runtime release.

The SNV index is a transformed representation of **Pangolin precomputed
scores**, created by Nils Wagner and Aleksandr Neverov and published under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) at
[Zenodo DOI 10.5281/zenodo.15649338](https://doi.org/10.5281/zenodo.15649338).
The runtime splice mask derives from GENCODE v38. Exact source identities,
transformations, citations, and redistribution notices are in
[`NOTICE`](NOTICE) and [`assets/notices/`](assets/notices/).

## Maintainers

- [Architecture overview](architecture/README.md)
- [Current project frontier](planning/frontier.md)
- [Development contract](AGENTS.md)

The separate maintenance executable lists authenticated build, verification,
packaging, and release-preparation commands without running them:

```bash
pangopup-build --help
```

Run the complete repository gate before committing:

```bash
make lint
make test
make spec
```
