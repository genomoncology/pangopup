# PangoPup

PangoPup is a fast, standalone, open-source service for
[Pangolin](https://github.com/tkzeng/Pangolin)-compatible splice scores on
GRCh38 variants. It has two paths:

- A single-nucleotide variant (SNV) is looked up in a precomputed,
  memory-mapped index.
- A supported lookup miss, insertion, deletion, or multi-nucleotide variant
  runs through the Pangolin model on the CPU.

Repeated model results are kept in a persistent SQLite cache. Results include
splice gain/loss scores, relative positions, matching gene records, and
provenance showing whether lookup or model produced them.

PangoPup does not interpret clinical significance, parse HGVS, project variants
to transcripts or proteins, or provide gene/disease knowledge.

These instructions describe the immutable **v0.3.0** application release. The
large biological assets are versioned separately and are unchanged from
v0.2.0.

## Storage and memory

Allow at least **25 GB free** for first synchronization. The downloaded files
are verified before installation, so temporary working space is needed.

| Component | Download | Installed |
|---|---:|---:|
| SNV lookup | 1,931,694,270 bytes (~1.80 GiB) | 15,033,158,255 bytes (~14.00 GiB) |
| Model, reference, mask | 691,874,664 bytes (~660 MiB) | 812,662,222 bytes (~775 MiB) |
| Combined | 2,623,568,934 bytes (~2.44 GiB) | 15,845,820,477 bytes (~14.76 GiB) |

The installed SNV file is intentionally larger than its compressed download.
It uses fixed-width records so PangoPup can jump directly to a score without
decompressing blocks. Linux memory-maps (`mmap`s) that file: mapping about
15 GB reserves virtual address space, but Linux reads only pages a query
touches. Mapped or file-backed bytes are not 15 GB of Rust heap and are not all
permanently resident in RAM.

One five-round, warm-page-cache observation on an AMD Ryzen 7 5825U running
Linux used the default one-worker/one-model-thread policy:

| Operation | Observed median / maximum |
|---|---:|
| Fresh one-SNV CLI | 5.9 / 6.2 ms; 12.0 / 12.3 MiB peak RSS |
| Ready service | 102.3 / 102.5 MiB PSS; 105.8 / 106.0 MiB RSS; 137.0 / 137.3 MiB high-water RSS |
| HTTP 1 / 10 / 100 SNVs | 0.8 / 0.5 / 1.1 ms median; 1.4 / 2.1 / 3.6 ms maximum |
| Uncached model request | 4.30 / 5.70 seconds |
| Fresh-service SQLite hit | 0.7 / 0.9 ms |

These are observations, not universal requirements or cold-cache guarantees.
RSS includes reclaimable mmap pages. Leave operational headroom; **256 MiB RAM
for one default service process** is a practical starting allocation, then
measure on your host and workload. Method, exact bytes, PSS/RSS definitions,
page-cache limitation, and raw identity are in
[`planning/artifacts/053-current-runtime-resources.md`](planning/artifacts/053-current-runtime-resources.md).

## Install on Linux

The direct executable supports Linux x86-64/amd64 with GLIBC 2.39 or newer.
It needs Bash, `curl` or `wget`, and `sha256sum`, `shasum`, or `openssl`.

Install the immutable v0.3.0 executable with:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
  | bash -s -- --version 0.3.0
export PATH="$HOME/.local/bin:$PATH"
```

The installer verifies the checksum, smoke-tests the executable, and writes
`${PANGOPUP_INSTALL_DIR:-$HOME/.local/bin}/pangopup` atomically. It does not use
`sudo`, edit `PATH`, or download scoring assets.

To build the exact release source, use Git, Rust 1.93, and a C/C++ toolchain:

```bash
git clone https://github.com/genomoncology/pangopup.git
cd pangopup
git checkout --detach v0.3.0
git rev-parse HEAD
cargo build --locked --release --package pangopup-cli
install -Dm755 target/release/pangopup "$HOME/.local/bin/pangopup"
```

Record the full commit from `git rev-parse HEAD` for reproducibility.

Download and verify the immutable scoring assets once:

```bash
pangopup sync --progress
pangopup status
pangopup sync --offline
```

Scoring is network-free after synchronization. Service startup never downloads
data; an operator must run `sync` explicitly.

Default paths follow XDG:

| Purpose | Default | Override |
|---|---|---|
| Installed assets | `~/.local/share/pangopup` | `PANGOPUP_DATA_DIR` or `XDG_DATA_HOME` |
| Resumable downloads | `~/.cache/pangopup` | `PANGOPUP_CACHE_DIR` or `XDG_CACHE_HOME` |
| SQLite model results | `~/.cache/pangopup/model-results.sqlite3` | `--model-cache`, `PANGOPUP_MODEL_CACHE`, then XDG |

`PANGOPUP_CACHE_DIR` relocates only resumable transport downloads; it does not
relocate SQLite. The model-result path precedence is `--model-cache`, then
`PANGOPUP_MODEL_CACHE`, then `$XDG_CACHE_HOME/pangopup/model-results.sqlite3`,
then `$HOME/.cache/pangopup/model-results.sqlite3`. Use absolute override paths.

## Score from the CLI

Variants use `GRCh38:CONTIG:POS:REF:ALT` with a 1-based position. Contigs may be
`1`–`22`, `X`, `Y`, `M`, their `chr` forms, or exact installed RefSeq
accessions.

```bash
# One SNV; JSON Lines is the default output.
pangopup lookup --variant GRCh38:chr12:6801301:G:A

# A batch in tab-separated form.
pangopup lookup \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr12:6801303:G:GA \
  --format table

# Deliberately bypass the SNV index and run the model.
pangopup lookup --model-only \
  --variant GRCh38:chr12:6801301:G:A
```

Add `--gene ENSG...` to retain one stable Ensembl gene ID. Automatic routing
uses a precomputed SNV hit and otherwise invokes the model.

In JSON, `provenance.kind` explains the route:

- `precomputed` means the stored Zenodo-derived SNV score was returned.
- `model` means the compatible model ran or its exact SQLite result was reused.

For example, this abridged model output shows the score records and route
(full output includes the exact asset identities):

```json
{"assembly":"GRCh38","contig":"chr12","position":6801303,"ref":"G","alt":"GA","status":"found","records":[{"gene":"ENSG00000010610.10","gain_score":"0.00","gain_position":0,"loss_score":"0.00","loss_position":-50,"warnings":[]}],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","effective_cpu_policy":"sequential:1/1","masked":true,"window":50}}
```

Scores are strings to retain their defined decimal representation. Loss scores
are signed. Overlapping genes can produce multiple records.

## Run the HTTP service

Run the foreground process on loopback:

```bash
pangopup serve --listen 127.0.0.1:8080
```

From another terminal:

```bash
curl -fsS http://127.0.0.1:8080/livez
curl -fsS http://127.0.0.1:8080/readyz
curl -fsS http://127.0.0.1:8080/v1/status

curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr12:6801303:G:GA"]}'

curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801303:G:GA"],"gene":"ENSG00000010610","model_only":true}'
```

The response is `{"results":[...]}` containing the same result records and
provenance as the CLI. A request accepts at most 100 variants and at most 10
uncached model variants. A full model queue returns HTTP 429.

The service has **no built-in authentication or TLS**. Keep it on loopback, or
put it behind an authenticated TLS reverse proxy with appropriate limits.
Docker, systemd, Kubernetes, or another process manager owns start, stop, and
restart; PangoPup does not daemonize itself.

## Docker on AMD64 and Apple Silicon

Docker selects the native AMD64 or ARM64 image:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.3.0
docker pull "$PANGOPUP_IMAGE"
docker volume create pangopup-data
docker volume create pangopup-cache

docker run --rm \
  -v pangopup-data:/var/lib/pangopup \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" sync --progress

docker run --rm --network none \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" status

docker run --rm --network none \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" lookup --variant GRCh38:chr12:6801301:G:A

docker run --rm --name pangopup -p 127.0.0.1:8080:8080 \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE"
```

From another terminal, exercise the running container:

```bash
curl -fsS http://127.0.0.1:8080/livez
curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A"]}'
```

The non-root image contains the executable and notices, not the scoring assets.
The data volume is durable and expensive to recreate; the cache volume contains
resumable downloads and disposable SQLite results. Verify an exact deployment
with `docker buildx imagetools inspect "$PANGOPUP_IMAGE"`, then deploy
`ghcr.io/genomoncology/pangopup@sha256:<INDEX_DIGEST>`.

Apple Silicon runs native Linux ARM64 code in Docker. Model inference uses the
ARM64 CPU, **not MPS or Metal**. ONNX Runtime may emit the harmless warning
`Unknown CPU vendor. cpuinfo_vendor value: 0`. Matched testing traced it to its
older CPU-identification data, not emulation or a scoring difference. PangoPup
will wait for an upstream ONNX Runtime release containing Apple-aware `cpuinfo`
rather than maintain a custom runtime only to hide the warning.

## Update and uninstall

Updating code does not redownload unchanged assets. To install or restore the
v0.3.0 direct executable:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
  | bash -s -- --version 0.3.0
```

Then confirm asset reuse:

```bash
pangopup sync --offline
pangopup status
```

For Docker, pull the new image, stop the foreground container, and replace it
while reusing both named volumes:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.3.0
docker pull "$PANGOPUP_IMAGE"
docker stop pangopup
docker run --rm --name pangopup -p 127.0.0.1:8080:8080 \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE"
```

For direct Linux installs, PangoPup checks and displays every resolved path
before removal:

```bash
pangopup uninstall              # choose code only, full removal, or cancel
pangopup uninstall --full       # full removal, still asks
pangopup uninstall --yes        # code only, noninteractive
pangopup uninstall --full --yes # code and managed data, noninteractive
```

Code-only removal preserves assets and cache for reinstall. Full removal also
removes managed XDG data, resumable downloads, and the default SQLite cache.
A `PANGOPUP_MODEL_CACHE` outside the managed cache root is not discoverable and
is not removed. Stop services first. The command rejects unsafe, aliased,
foreign-owned, busy, or unexpected paths and removes the executable last.

Docker lifecycle remains host-managed:

```bash
docker image rm ghcr.io/genomoncology/pangopup:0.3.0
docker volume rm pangopup-cache # downloads and model-result cache
docker volume rm pangopup-data  # large installed assets
```

## Limitations, security, and licenses

- GRCh38 only; no GRCh37/liftover, transcript HGVS, or clinical interpretation.
- CPU ONNX inference only; no MPS, Metal, CUDA, or GPU path.
- No built-in HTTP authentication, TLS, or process supervisor.
- Precomputed lookup reproduces the published dataset; `--model-only` exists
  for an explicit current-model comparison.

PangoPup is [GPL-3.0-only](LICENSE). Its converted runtime derives from Pangolin
1.0.2 and retains the required GPL source and modification material. The SNV
index transforms **Pangolin precomputed scores** by Nils Wagner and Aleksandr
Neverov, published under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
at [Zenodo DOI 10.5281/zenodo.15649338](https://doi.org/10.5281/zenodo.15649338).
The mask derives from GENCODE v38. See [`NOTICE`](NOTICE) and
[`assets/notices/`](assets/notices/) for exact identities and attribution.

Maintainers: [Architecture overview](architecture/README.md),
[Current project frontier](planning/frontier.md), and
[development contract](AGENTS.md). Run `pangopup-build --help` for authenticated
maintenance commands and `make lint`, `make test`, `make spec` before commits.
