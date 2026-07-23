# Artifact Delivery

This document records shipped local transport and the accepted managed-delivery
target. The repository does not yet publish generated lookup/model assets and
does not yet implement an asset manager, automatic installation, or XDG
discovery. The shipped runtime opens an explicitly supplied bundle path.

## GitHub Releases

[GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
are the initial distribution channel. GitHub currently permits up to 1,000
assets on one release, requires each asset to be under 2 GiB, and states no
aggregate release-size or bandwidth quota. Ordinary Git objects remain subject
to the 100 MiB limit, so generated indices and model weights never enter Git
history or Git LFS.

The initial release family should keep independently versioned concerns
separate. The SNV lookup is a logical transport set, not one lookup archive:

```text
pangopup-<version>-<target>.tar.zst
SNV transport set:
  transport.json
  bundle-manifest.json
  NOTICE
  payload.pgi.zst.part0000
  payload.pgi.zst.part0001
  ...
pangopup-models-<upstream-version>-<conversion>.tar.zst
pangopup-reference-grch38p14-<format>.tar.zst
pangopup-mask-gencode38-<format>.tar.zst
SHA256SUMS
release-manifest.json
```

The SNV transport compresses only the exact `scores.pgi` byte stream as one
deterministic Zstandard frame, then cuts that stream into ordered
1,000,000,000-byte parts. It carries canonical transport metadata plus exact
copies of the installed bundle manifest and CC BY notice. It does not put the
three-file bundle in tar and does not alter the reconstructed fixed-v1 member.
How the future publication manifest namespaces those logical filenames as
GitHub assets is intentionally left to the publication slice.

The local representation and commands are shipped now:

```text
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
```

Pack first exhaustively certifies the installed bundle. Integrity-only verify
streams every declared layer and proves the exact decompressed member without
creating it; it does not authenticate the publisher or prove fixed-v1 semantic
structure. Unpack writes into unique same-filesystem staging, runs complete
semantic certification, syncs it, and publishes by Linux no-replace rename.

The model archive contains only the exact checkpoints needed by the supported
inference implementation plus upstream notices and checksums. Keeping concerns
separate lets lookup-only installations avoid model bytes and lets data, model,
reference, mask, and executable releases evolve without pretending they share
a version. The reference and mask assets are optional unless model fallback is
enabled.

Every published asset name is immutable and content-addressed by the release
manifest. Enable GitHub immutable releases if the repository setting is
available. Never replace an asset in place; issue a new release and identity.

## Delivery stages

Delivery is proved in layers so network behavior cannot hide an invalid local
format or installer:

1. deterministic pack, split, verify, and byte-identical unpack of a supplied
   certified lookup bundle;
2. local installation of supplied parts with platform-directory discovery,
   locking, staging, checksums, receipts, atomic publication, and verified
   reuse;
3. pinned remote sync with resumable downloads into the same installer; and
4. immutable GitHub publication plus a clean-machine install, offline restart,
   and representative query proof.

The first stage is shipped. Each later stage
receives its own coordinator-authored and independently reviewed contract only after the
preceding transport or installation contract is implemented.

## Planned installation

Each future distribution embeds or ships a lock manifest for one compatible
asset set. The target default full profile includes lookup data, model,
reference, and masking assets; an explicit lookup-only profile omits model
fallback.

The future CLI and HTTP service will ensure the selected profile before opening
the runtime. The shared asset manager should:

1. resolve the binary-pinned or explicitly requested asset set, never an
   unpinned “latest” release;
2. take a per-bundle installation lock;
3. reuse a complete compatible local bundle without network access;
4. otherwise download missing archives to a temporary cache path;
5. verify size and SHA-256 before extraction;
6. extract into a sibling staging directory;
7. verify the inner manifest, member sizes, formats, and hashes;
8. atomically publish the completed immutable directory.

The future `pangopup assets install` command will install caller-supplied parts.
The future `pangopup assets sync` command will fetch one explicitly pinned
release manifest and then call the same installer. Offline mode will forbid
network access and name every missing or incompatible asset. Callers will still
be able to supply an already installed bundle path. Containers will be able to
bake the same verified bundle into an image or mount it read-only.

Default managed storage will follow the operating system's application-data convention
rather than Pangolin's current Python-package layout: XDG data storage on Linux,
Application Support on macOS, and the equivalent known folder on Windows. An
explicit CLI flag or environment variable will override discovery. The core library
accepts paths and performs no download or home-directory discovery.

On Linux, durable installed bundles will live under
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`; transport archives and partial
downloads may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. Installed data
will not be cache: clearing the cache must not break a complete installation.

The explicit local installer does not require a network and is the primitive.
Remote sync only obtains the exact bytes named by a pinned manifest and then
calls that installer. A complete installed profile remains usable offline even
if the remote release is unavailable.

Full hashes will be mandatory during installation and explicit verification.
Ordinary startup will validate the trusted manifest identity, sizes, format
versions, and structures without rereading every byte. This will keep startup
cheap and avoid loading the whole mapped corpus merely to prove it has not
changed.

## Historical compression evidence and shipped transport

The certified installed member is 15,033,158,255 bytes. GNU tar 1.35 plus
Zstandard 1.5.5 level 9 produced a 1,935,000,209-byte transport archive
(`sha256:3e87d80fdad963ca6ffca646393b8bb3955214b77cd8b7f1782e48d039aba751`).
That was a historical size experiment, not the runtime or release format.
Although it fell below GitHub's under-2-GiB per-file ceiling, the remaining
headroom was too small for a robust single asset and tar would add large-file
format choices unrelated to lookup data.

The shipped implementation instead streams only `scores.pgi` through a pinned
Zstandard encoder, hashes the complete compressed stream, and divides it into
deterministic one-billion-byte parts. A canonical manifest binds part order,
sizes, hashes, encoder identity, the complete stream, copied small members, and
the exact reconstructed score identity. Verification streams across parts;
unpack publishes the unchanged three-file fixed-v1 bundle. Partial or mixed
installs fail closed. Model, reference, and masking assets remain separately
versioned.

## Publication acceptance

Uploading files is not sufficient publication evidence. A release is accepted
only when automation or a retained run proves, from a clean supported machine:

- the executable and manifest identities are explicit;
- every part downloads or is supplied locally and verifies by size and hash;
- installation publishes exactly the expected immutable members and notices;
- an offline second start reuses the installation without remote access;
- representative JSONL and table lookups match the retained oracle; and
- corruption, a missing part, and mixed-release parts fail closed without
  replacing a previous valid installation.

Future model/reference/mask publication extends the same proof with pinned
compatibility-corpus inference. Signing, SBOM, build provenance, and rollback
policy are later release-hardening outcomes, not shipped claims.
