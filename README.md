# PangoPup

PangoPup provides fast, local [Pangolin](https://github.com/tkzeng/Pangolin)-compatible
splice predictions for GRCh38 variants. It reports the strongest predicted splice-site
gain and signed loss caused by a variant, together with each result's genomic-coordinate
offset. The scores help identify variants that may alter RNA splicing.

For single-nucleotide variants (SNVs), PangoPup first checks a memory-mapped index of
published scores. A supported lookup miss or non-SNV runs through the Pangolin model on
the CPU, and modeled results are saved in SQLite for reuse. The reported search region is
50 bases on either side of the variant; for a deletion, its allele span can extend the
positive offset beyond 50.

## Quick start

The direct executable requires Linux x86-64/amd64 with GLIBC 2.39 or newer. The first
sync downloads about 2.44 GiB, installs about 14.76 GiB, and needs at least 25 GB free.

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
  | bash -s -- --version 0.3.0
export PATH="$HOME/.local/bin:$PATH"

pangopup sync --progress
pangopup status
pangopup lookup --variant GRCh38:chr12:6801301:G:A
pangopup lookup --variant GRCh38:chr12:6801303:G:GA
```

After `sync`, scoring is network-free. The SNV normally uses the precomputed index; the
supported insertion automatically uses the model. Both commands return JSON Lines.

## Input and output

Variants use `GRCh38:CONTIG:POS:REF:ALT` with a 1-based genomic position. Accepted
contigs are `1`–`22`, `X`, `Y`, `M`, their `chr` forms, or the corresponding installed
RefSeq accessions. Alleles must be nonempty uppercase strings containing only A, C, G,
or T.

PangoPup uses the submitted representation exactly; it does not trim, align, or
normalize alleles. Insertions and deletions use an anchored form: the REF and ALT alleles
share the first base, and one allele is one base long. Model scoring checks REF against the
installed GRCh38 reference and accepts at most 100 bases in either allele. Equal-length
substitutions are also supported.

```bash
# Score a batch and render a tab-separated table.
pangopup lookup \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr12:6801303:G:GA \
  --format table

# Keep records for one stable Ensembl gene ID.
pangopup lookup --variant GRCh38:chr12:6801301:G:A \
  --gene ENSG00000010610

# Bypass the SNV index and run the model explicitly.
pangopup lookup --model-only \
  --variant GRCh38:chr12:6801301:G:A
```

JSON Lines is the default format. Each result contains a `status`, zero or more gene
`records`, and provenance. Overlapping genes can produce multiple records. Status means:

- `found`: at least one score record was produced with no source-reference ambiguity.
- `not_found`: no score record was produced for the request and optional gene filter;
  this is not a prediction of zero effect.
- `ambiguous_source_reference`: the precomputed source used `N` as its reference at the
  locus, so its source-associated gene, published alternate alleles, and omitted alternate are reported instead of a score.
- `mixed`: score records and a source-reference ambiguity both occurred, which can happen
  across overlapping genes.

A score record contains `gain_score`, `gain_position`, `loss_score`, and `loss_position`.
Loss is signed, and positions are offsets from the submitted genomic position. A positive
position always means a higher genomic coordinate, including for a minus-strand gene.
`provenance.kind` is `precomputed` for an index result or `model` for model inference or
an exact SQLite reuse.

Run `pangopup <command> --help` for command-specific options.

## HTTP service

Start the foreground service on loopback:

```bash
pangopup serve --listen 127.0.0.1:8080
```

```bash
curl -fsS http://127.0.0.1:8080/readyz

curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr12:6801303:G:GA"]}'

curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A"],"model_only":true}'
```

Health and status routes are `/livez`, `/readyz`, and `/v1/status`. `/v1/score` accepts
1–100 variants per request and at most 10 uncached model variants. A full model queue
returns HTTP 429. The service has no built-in authentication or TLS: keep it on
loopback, or place it behind an authenticated TLS reverse proxy with suitable limits.
Use Docker, systemd, Kubernetes, or another process manager when the foreground process
needs supervised start, stop, and restart behavior.

## Docker

The published image supports native Linux AMD64 and ARM64. Create persistent volumes,
sync once, then start the service:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.3.0
docker pull "$PANGOPUP_IMAGE"
docker volume create pangopup-data
docker volume create pangopup-cache

docker run --rm \
  -v pangopup-data:/var/lib/pangopup \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" sync --progress

docker run --rm --name pangopup -p 127.0.0.1:8080:8080 \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE"
```

Use the image as a CLI without starting the server:

```bash
docker run --rm --network none \
  -v pangopup-data:/var/lib/pangopup:ro \
  -v pangopup-cache:/var/cache/pangopup \
  "$PANGOPUP_IMAGE" lookup --variant GRCh38:chr12:6801301:G:A
```

Removing or replacing a container preserves both named volumes. Apple Silicon runs the
ARM64 image through Docker Desktop; model inference is CPU-only and does not use MPS or
Metal.

## Storage and operations

| Component | Download | Installed |
|---|---:|---:|
| SNV lookup | ~1.80 GiB | ~14.00 GiB |
| Model, reference, and mask | ~660 MiB | ~775 MiB |
| Combined | ~2.44 GiB | ~14.76 GiB |

The SNV index is memory-mapped rather than loaded wholly into RAM. Linux reads the file
pages touched by queries and can reclaim them through its normal page cache. For one
default foreground service, 256 MiB RAM is a practical starting allocation; measure
against your workload before setting a production limit.

By default, installed assets are in `~/.local/share/pangopup`; resumable downloads are in
`~/.cache/pangopup`; and model results are in `~/.cache/pangopup/model-results.sqlite3`.

Verify an existing installation without network access:

```bash
pangopup sync --offline
pangopup status
```

To install a chosen release, use its version in both the URL and installer argument:

```bash
VERSION=0.3.0
curl -fsSL "https://raw.githubusercontent.com/genomoncology/pangopup/v${VERSION}/install.sh" \
  | bash -s -- --version "$VERSION"
```

This replaces the executable while preserving assets and caches. Run
`pangopup sync --offline` to confirm reuse, or `pangopup sync --progress` when the chosen
release requires different assets.

For Docker, pull the chosen tag, stop the container started above, then repeat the same
service command with both named volumes:

```bash
export PANGOPUP_IMAGE=ghcr.io/genomoncology/pangopup:0.3.0
docker pull "$PANGOPUP_IMAGE"
docker stop pangopup
```

Replacing the container preserves the data and cache volumes.

For removal, PangoPup displays every resolved path before asking what to delete:

```bash
pangopup uninstall              # choose code only, full removal, or cancel
pangopup uninstall --yes        # code only, without prompting
pangopup uninstall --full       # code, managed data, and cache; asks first
pangopup uninstall --full --yes # full removal without prompting
```

For Docker, remove code with `docker image rm ghcr.io/genomoncology/pangopup:0.3.0`.
Optionally remove downloads and cached model results with
`docker volume rm pangopup-cache`, or installed assets with
`docker volume rm pangopup-data`.

## Citation and license

To cite PangoPup, use [`CITATION.cff`](CITATION.cff). PangoPup is
[GPL-3.0-only](LICENSE). Exact source identities, modifications, and attribution are in
[`NOTICE`](NOTICE) and [`assets/notices/`](assets/notices/).

PangoPup builds on these works:

- **Pangolin software** — Tony Zeng,
  [GPL-3.0 source](https://github.com/tkzeng/Pangolin).
- **Pangolin paper** — Tony Zeng and Yang I. Li, “Predicting RNA splicing from DNA
  sequence using Pangolin,” *Genome Biology* 23, 103 (2022),
  [DOI 10.1186/s13059-022-02664-4](https://link.springer.com/article/10.1186/s13059-022-02664-4).
- **Pangolin precomputed scores** — Nils Wagner and Aleksandr Neverov,
  [Zenodo DOI 10.5281/zenodo.15649338](https://doi.org/10.5281/zenodo.15649338),
  licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
- **GENCODE v38** — source annotation used for splice-site masking; see
  [`NOTICE`](NOTICE) for the pinned source and terms.
