# Artifact Delivery

This document records the accepted target delivery design. The repository does
not yet publish generated lookup/model assets and does not yet implement an
asset manager, automatic installation, or XDG discovery. The shipped CLI opens
an explicitly supplied bundle path.

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

The SNV transport assets contain the fixed 11-byte lookup bundle, its CC BY 4.0
attribution, source DOI/checksum, GRCh38 compatibility evidence, and format metadata. The
model archive contains only the exact checkpoints needed by the supported
inference implementation plus upstream notices and checksums. Keeping it
separate lets lookup-only installations avoid model bytes and lets data, model,
reference, mask, and executable releases evolve without pretending they share
a version. The reference and mask assets are optional unless model fallback is
enabled.

Every published asset name is immutable and content-addressed by the release
manifest. Enable GitHub immutable releases if the repository setting is
available. Never replace an asset in place; issue a new release and identity.

## Planned installation

Each future distribution embeds or ships a lock manifest for one compatible
asset set. The target default full profile includes lookup data, model,
reference, and masking assets; an explicit lookup-only profile omits model
fallback.

The future CLI and HTTP service will ensure the selected profile before opening
the runtime. The shared asset manager should:

1. resolve the binary-pinned or explicitly requested asset set;
2. take a per-bundle installation lock;
3. reuse a complete compatible local bundle without network access;
4. otherwise download missing archives to a temporary cache path;
5. verify size and SHA-256 before extraction;
6. extract into a sibling staging directory;
7. verify the inner manifest, member sizes, formats, and hashes;
8. atomically publish the completed immutable directory.

The future `pangopup assets install` command will expose the same operation for
prefetching. Offline mode will forbid network access and name every missing or
incompatible asset. Callers will still be able to supply an already installed
bundle path. Containers will be able to bake the same verified bundle into an
image or mount it read-only.

Default managed storage will follow the operating system's application-data convention
rather than Pangolin's current Python-package layout: XDG data storage on Linux,
Application Support on macOS, and the equivalent known folder on Windows. An
explicit CLI flag or environment variable will override discovery. The core library
accepts paths and performs no download or home-directory discovery.

On Linux, durable installed bundles will live under
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`; transport archives and partial
downloads may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. Installed data
will not be cache: clearing the cache must not break a complete installation.

Full hashes will be mandatory during installation and explicit verification.
Ordinary startup will validate the trusted manifest identity, sizes, format
versions, and structures without rereading every byte. This will keep startup
cheap and avoid loading the whole mapped corpus merely to prove it has not
changed.

## Why split SNV transport from the start

The certified installed member is 15,033,158,255 bytes. GNU tar 1.35 plus
Zstandard 1.5.5 level 9 produced a 1,935,000,209-byte transport archive
(`sha256:3e87d80fdad963ca6ffca646393b8bb3955214b77cd8b7f1782e48d039aba751`).
Although that measured archive is below GitHub's under-2-GiB per-file ceiling,
the remaining headroom is too small for a robust release contract. Package it
as deterministic split transport assets pinned by one manifest, then reassemble
the exact installed fixed-v1 member. Partial or mixed installs fail closed.
Model, reference, and masking assets remain separately versioned.
