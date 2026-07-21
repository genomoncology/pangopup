# Artifact Delivery

## GitHub Releases

[GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
are the initial distribution channel. GitHub currently permits up to 1,000
assets on one release, requires each asset to be under 2 GiB, and states no
aggregate release-size or bandwidth quota. Ordinary Git objects remain subject
to the 100 MiB limit, so generated indices and model weights never enter Git
history or Git LFS.

The initial release family should keep independently versioned concerns
separate:

```text
pangopup-<version>-<target>.tar.zst
pangopup-snv-grch38-<dataset-id>-<format>.tar.zst
pangopup-models-<upstream-version>-<conversion>.tar.zst
pangopup-reference-grch38p14-<format>.tar.zst
pangopup-mask-gencode38-<format>.tar.zst
SHA256SUMS
release-manifest.json
```

The SNV archive contains the sparse lookup bundle, its CC BY 4.0 attribution,
source DOI/checksum, GRCh38 compatibility evidence, and format metadata. The
model archive contains only the exact checkpoints needed by the supported
inference implementation plus upstream notices and checksums. Keeping it
separate lets lookup-only installations avoid model bytes and lets data, model,
reference, mask, and executable releases evolve without pretending they share
a version. The reference and mask assets are optional unless model fallback is
enabled.

Every published asset name is immutable and content-addressed by the release
manifest. Enable GitHub immutable releases if the repository setting is
available. Never replace an asset in place; issue a new release and identity.

## Installation

Each binary embeds or ships a lock manifest for one compatible asset set. The
default full profile includes lookup data, model, reference, and masking assets;
an explicit lookup-only profile omits model fallback.

The CLI and HTTP service automatically ensure the selected profile before
opening the runtime. The shared asset manager should:

1. resolve the binary-pinned or explicitly requested asset set;
2. take a per-bundle installation lock;
3. reuse a complete compatible local bundle without network access;
4. otherwise download missing archives to a temporary cache path;
5. verify size and SHA-256 before extraction;
6. extract into a sibling staging directory;
7. verify the inner manifest, member sizes, formats, and hashes;
8. atomically publish the completed immutable directory.

`pangopup assets install` exposes the same operation for prefetching. Offline
mode forbids network access and names every missing or incompatible asset.
Callers can instead supply an already installed bundle path. Containers may
bake the same verified bundle into an image or mount it read-only.

Default storage follows the operating system's application-data convention
rather than Pangolin's current Python-package layout: XDG data storage on Linux,
Application Support on macOS, and the equivalent known folder on Windows. An
explicit CLI flag or environment variable overrides discovery. The core library
accepts paths and performs no download or home-directory discovery.

On Linux, durable installed bundles live under
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`; transport archives and partial
downloads may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. Installed data is
not cache: clearing the cache must not break a complete installation.

Full hashes are mandatory during installation and explicit verification.
Ordinary startup validates the trusted manifest identity, sizes, format
versions, and structures without rereading every byte. This keeps startup cheap
and avoids loading the whole mapped corpus merely to prove it has not changed.

## Why one SNV archive first

A single full-SNV transport archive is simplest and remains comfortably below
the current per-asset limit after compression. Model, reference, and masking
assets remain separately versioned. The internal SNV bundle may still use one
member or per-contig members according to measured lookup behavior. If the final
archive loses sufficient headroom, per-contig release assets are a packaging
fallback; all are pinned by one manifest so partial or mixed installs fail
closed.
