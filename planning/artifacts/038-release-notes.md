# Pangopup v0.1.0

This is Pangopup's first Linux x86_64 executable release. It provides:

- memory-mapped lookup of precomputed Pangolin scores for GRCh38 SNVs;
- Pangolin-compatible CPU inference for supported SNV misses and non-SNVs;
- checksum-verified synchronization of the separately published immutable SNV
  and model-side runtime assets;
- persistent SQLite caching of model results; and
- JSON Lines and tab-separated CLI output.

The executable supports Linux x86_64/amd64 with GLIBC 2.39 or newer. Install
the immutable version with:

```bash
curl -fsSL https://raw.githubusercontent.com/genomoncology/pangopup/v0.1.0/install.sh | bash -s -- --version 0.1.0
pangopup sync
pangopup status
```

The installer requires Bash, curl or wget, and sha256sum, shasum, or openssl.
It installs without sudo under `$HOME/.local/bin` unless
`PANGOPUP_INSTALL_DIR` is set. It does not edit shell configuration or download
runtime data. `pangopup sync` installs the pinned data separately under the
Linux XDG data directory and reuses verified complete data offline.

The release contains exactly the executable, its SHA-256 file, a CycloneDX
SBOM, the executable release manifest, `LICENSE`, and `NOTICE`. The Pangolin
model-side runtime and precomputed-score lookup data remain separate immutable
releases with their own licenses and notices.
