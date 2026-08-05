# PangoPup v0.3.0 release notes

## User-visible changes

- `pangopup uninstall` now checks and displays the resolved executable, data,
  and cache paths before offering code-only removal, complete removal, or
  cancellation. `--full` selects complete removal and `--yes` makes either
  scope noninteractive.
- `pangopup status` works with a read-only installed-data mount.
- Every runtime command and asset namespace has focused `-h` and `--help`.
- Multi-gigabyte synchronization has bounded transient retries, safe resume,
  optional phase/byte progress, and a quiet mode without changing final JSON.
- The first-use README is shorter and now includes exact download/installed
  sizes plus measured Linux memory and latency guidance.

## Compatibility

This is an application-code release. It does not rebuild or replace the
immutable `snv-grch38-v1` or `runtime-grch38-v1` assets. The precomputed SNV
scores, Pangolin model, compiled RefSeq GRCh38.p14 reference, GENCODE v38 mask,
score semantics, signed loss values, lookup-first default, and explicit
`--model-only` behavior are unchanged from v0.2.0.

PangoPup remains available as a direct Linux x86-64 executable and as a thin,
non-root Linux AMD64/ARM64 container. The container contains application code
and notices only; users synchronize the separately versioned assets into a
persistent volume.

## Resource guidance

The immutable downloads total 2,623,568,934 bytes and install 15,845,820,477
core bytes. One five-round warm-page-cache observation on an AMD Ryzen 7 5825U
found a one-SNV CLI peak of 12.0/12.3 MiB RSS (median/maximum) and the default
eager-model service at 102.3/102.5 MiB PSS, with 137.0/137.3 MiB high-water RSS.
Those are single-host
observations rather than universal minimums. Full method and limitations are
retained in
[`053-current-runtime-resources.md`](https://github.com/genomoncology/pangopup/blob/v0.3.0/planning/artifacts/053-current-runtime-resources.md).

## Known limitation

On Apple Silicon under Docker, model inference uses the ARM64 CPU rather than
MPS or Metal. The qualified ONNX Runtime can print
`onnxruntime cpuid_info warning: Unknown CPU vendor. cpuinfo_vendor value: 0`.
A matched A/B experiment traced this harmless warning to ONNX Runtime's older
CPU-identification dependency. PangoPup is waiting for an upstream release
with Apple-aware identification rather than carrying a custom runtime solely
to suppress the message.

## Installation

The immutable Linux x86-64 executable installer is:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh \
  | bash -s -- --version 0.3.0
```

The thin native AMD64/ARM64 container is
`ghcr.io/genomoncology/pangopup:0.3.0`. Both delivery forms identify the same
source revision. Scoring assets remain separate and are installed by
`pangopup sync`.
