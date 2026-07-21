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
pangopup-data-grch38-<dataset-id>-<format>.tar.zst
pangopup-models-<upstream-version>-<conversion>.tar.zst
pangopup-reference-grch38p14-<format>.tar.zst
pangopup-mask-gencode38-<format>.tar.zst
SHA256SUMS
release-manifest.json
```

The data archive contains the sparse lookup bundle, its CC BY 4.0 attribution,
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

`pangopup assets install` is the intended convenience adapter, not a hidden
runtime downloader. It should:

1. resolve an explicitly requested or lockfile-pinned asset set;
2. download to a temporary file;
3. verify size and SHA-256 before extraction;
4. extract into a sibling staging directory;
5. verify the inner bundle manifest, member sizes, and hashes;
6. atomically publish the completed immutable directory.

The caller can instead supply an already installed bundle path. Containers may
bake the same verified bundle into an image or mount it read-only.

Default storage follows the operating system's application-data convention
rather than Pangolin's current Python-package layout: XDG data storage on Linux,
Application Support on macOS, and the equivalent known folder on Windows. An
explicit CLI flag or environment variable overrides discovery. The core library
accepts paths and performs no download or home-directory discovery.

## Why one archive first

A single full-data transport archive is simplest and remains comfortably below
the current per-asset limit after compression. The internal bundle may still use
one member or per-contig members according to measured lookup behavior. If the
final archive loses sufficient headroom, per-contig release assets are a
packaging fallback; all are pinned by one manifest so partial or mixed installs
fail closed.
