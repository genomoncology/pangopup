# PangoPup v0.2.0

PangoPup v0.2.0 is the qualified Linux x86-64 release of the complete
lookup-first splice-scoring product. It preserves the scoring formats, model,
reference, splice mask, and immutable SNV/runtime asset identities used by
v0.1.0 while adding the user-facing surfaces developed since that release.

## What changed

- Explicit `pangopup lookup --model-only` scoring bypasses the SNV index.
- `pangopup serve` provides bounded foreground HTTP scoring plus `/livez`,
  `/readyz`, and `/v1/status`.
- `pangopup sync --progress` reports phases and byte progress; safe partial
  downloads resume, transient failures retry four times, and `--quiet` keeps
  stderr silent while preserving the final JSON result.
- `sync`, `status`, `lookup`, `serve`, and asset namespaces have focused `-h`
  and `--help`.
- `status` works with a read-only installed-data mount.
- Automatic model results and explicit model-only results reuse the persistent
  exact SQLite cache.

The biological contract is unchanged: an indexed GRCh38 SNV returns its exact
precomputed Pangolin score; a supported miss or non-SNV runs the compatible
Pangolin model. PangoPup reports splice effects and provenance, not clinical
significance.

## Install and synchronize

The executable requires Linux x86-64/amd64 with GLIBC 2.39 or newer, Bash,
`curl` or `wget`, and `sha256sum`, `shasum`, or `openssl`.

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.2.0/install.sh \
  | bash -s -- --version 0.2.0
export PATH="$HOME/.local/bin:$PATH"
pangopup sync --progress
pangopup status
```

Allow at least 25 GB free for the first synchronization. Assets install under
the XDG user-data directory and resumable downloads/model results use the XDG
cache directory. The installer downloads only the executable and checksum;
asset synchronization remains explicit.

## Use

```bash
pangopup lookup --variant GRCh38:chr12:6801301:G:A
pangopup lookup --model-only --variant GRCh38:chr12:6801301:G:A
pangopup serve --listen 127.0.0.1:8080
curl -fsS http://127.0.0.1:8080/v1/score \
  -H 'content-type: application/json' \
  --data '{"variants":["GRCh38:chr12:6801301:G:A"]}'
```

The service has no built-in TLS or authentication. Keep it on loopback or put
it behind an authenticated TLS reverse proxy.

## Platforms and assets

This release contains one direct Linux x86-64 executable, its checksum,
CycloneDX SBOM, canonical manifest, `LICENSE`, and `NOTICE`. It does not
republish raw Zenodo, NCBI, or GENCODE inputs and does not change or republish
the immutable `snv-grch38-v1` or `runtime-grch38-v1` asset releases.

The repository Dockerfile builds native Linux AMD64 and ARM64 images, including
under Docker Desktop on Apple Silicon, but no registry image is part of this
release. Apple inference is CPU-only and does not use MPS/Metal. The accepted
ONNX Runtime may print a harmless unknown-CPU-vendor warning on Apple Docker.

PangoPup is GPL-3.0-only. The transformed SNV index retains attribution to
Nils Wagner and Aleksandr Neverov's CC BY 4.0 *Pangolin precomputed scores*
(DOI 10.5281/zenodo.15649338). The runtime retains Pangolin source, model, and
license material. See `NOTICE` and the installed asset notices for exact
provenance.
