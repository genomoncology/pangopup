# Artifact Delivery

This document records the shipped immutable SNV release, local transport,
Linux installation, and accepted remote-sync target. The repository does not
yet implement remote sync/download. The runtime opens either
an explicitly supplied bundle path or the active receipt-bound bundle in Linux
user data.

## GitHub Releases

[GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
are the initial distribution channel. GitHub currently permits up to 1,000
assets on one release, requires each asset to be under 2 GiB, and states no
aggregate release-size or bandwidth quota. Ordinary Git objects remain subject
to the 100 MiB limit, so generated indices and model weights never enter Git
history or Git LFS.

The initial release family should keep independently versioned concerns
separate. The SNV lookup uses one eight-file release asset set whose installable
transport is the closed five-file subset:

```text
pangopup-<version>-<target>.tar.zst
SNV release asset set:
  Installable transport (exactly five files):
    transport.json
    bundle-manifest.json
    NOTICE
    payload.pgi.zst.part0000
    payload.pgi.zst.part0001
  Publication metadata (kept outside the transport directory):
    proof-receipt.json
    release-profile.json
    SHA256SUMS
pangopup-models-<upstream-version>-<conversion>.tar.zst
pangopup-reference-grch38p14-<format>.tar.zst
pangopup-mask-gencode38-<format>.tar.zst
```

The SNV transport compresses only the exact `scores.pgi` byte stream as one
deterministic Zstandard frame, then cuts that stream into ordered
1,000,000,000-byte parts. It carries canonical transport metadata plus exact
copies of the installed bundle manifest and CC BY notice. It does not put the
three-file bundle in tar and does not alter the reconstructed fixed-v1 member.
The checked `snv-grch38-v1` release profile fixes the GitHub asset names, sizes,
digests, and literal immutable-release URLs for those members.
That contract is published at
[`snv-grch38-v1`](https://github.com/genomoncology/pangopup/releases/tag/snv-grch38-v1).
The release reports `immutable=true`; its exact eight names, sizes, and
server-side SHA-256 digests match the checked profile.

The local representation and commands are shipped now:

```text
pangopup-build transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
pangopup-build transport verify --transport <TRANSPORT_DIR>
pangopup-build transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
pangopup-build release prepare --transport <TRANSPORT_DIR> --receipt <PROOF_RECEIPT_JSON> --output <ABSENT_DIR>
pangopup-build release upload-asset --transport <TRANSPORT_DIR> --prepared <PREPARED_DIR> --gh <ABSOLUTE_PINNED_GH_BINARY> --release-id <POSITIVE_GITHUB_ID> --asset <EXACT_ASSET_NAME>
```

Pack first exhaustively certifies the installed bundle. Integrity-only verify
streams every declared layer and proves the exact decompressed member without
creating it; it does not authenticate the publisher or prove fixed-v1 semantic
structure. Unpack writes into unique same-filesystem staging, runs complete
semantic certification, syncs it, and publishes by Linux no-replace rename.
Release preparation inspects only the three bounded metadata files and
no-follow name/type/size metadata for payload parts. It emits the reviewed
profile, receipt copy, digest list, and release notes atomically; it performs no
network or publication action and never opens a part.
The upload command is a coordinator-only publication tool, not runtime remote
sync. It accepts one of the eight reviewed asset names, validates and executes
an immutable sealed snapshot of the official GitHub CLI 2.45.0, and streams one
stable selected source after revalidating both closed local directories. Small
assets are copied into sealed memfds. Large payloads remain unread by the
parent and are protected by a monitored Linux read lease, explicit SIGIO
routing, a final lease check, and zero-offset/content-blind syscall boundary.
The 21,600-second request deadline kills the child's process group and reaps
the direct child; lease-break cleanup has a separate five-second ceiling.
One close-on-exec signal descriptor supervises blocked `SIGINT`, `SIGTERM`, and
`SIGIO`; pending signals are drained before spawn, and orderly interruption
uses the same group-kill, direct-reap, and lease-release path. The direct child
sets a parent-death `SIGKILL`, verifies the parent PID captured before spawn,
restores the original signal mask, and only then executes the sealed CLI. The
command never resolves the selected pathname again, invokes a shell, retries,
or reads credentials itself. The reviewed executable comes from source commit
`3ca179bcdeb46b5e54ddc6cad8feb6addf487d7c` and the 10,716,793-byte
`gh_2.45.0_linux_amd64.tar.gz` archive with
`sha256:79e89a14af6fc69163aee00e764e86d5809d0c6c77e6f229aebe7a4ed115ee67`.
The command itself requires the extracted executable to be exactly 43,495,424
bytes with
`sha256:d4a46368912cfc7b9f0a897a613910e34562ef033fc6029e0bea52c43b440fa4`.

The model archive contains only the exact checkpoints needed by the supported
inference implementation plus upstream notices and checksums. Keeping concerns
separate lets lookup-only installations avoid model bytes and lets data, model,
reference, mask, and executable releases evolve without pretending they share
a version. The reference and mask assets are optional unless model fallback is
enabled.

Every published asset name is immutable and content-addressed by the release
manifest. GitHub immutable releases are mandatory: publication is blocked
unless the repository setting is enabled and the completed release reports
`immutable=true`. A mutable release is not an acceptable fallback. Never
replace an asset in place; issue a new release and identity.

## Delivery stages

Delivery is proved in layers so network behavior cannot hide an invalid local
format or installer:

1. deterministic pack, split, verify, and byte-identical unpack of a supplied
   certified lookup bundle;
2. local installation of supplied parts with platform-directory discovery,
   locking, staging, checksums, receipts, atomic publication, and verified
   reuse;
3. immutable GitHub publication plus bounded unauthenticated verification and
   a documented exact manual-install path; and
4. pinned remote sync with resumable downloads into the same installer.

The first three stages are shipped. Each later stage
receives its own coordinator-authored and independently reviewed contract only after the
preceding transport or installation contract is implemented.

## Shipped Linux installation and planned remote sync

Each future remote distribution embeds or ships a lock manifest for one compatible
asset set. The target default full profile includes lookup data, model,
reference, and masking assets; an explicit lookup-only profile omits model
fallback.

The shipped local command accepts an already available transport:

```text
pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
pangopup assets status [--data-dir <ABSOLUTE_PATH>]
```

It resolves `--data-dir`, `PANGOPUP_DATA_DIR`, `XDG_DATA_HOME/pangopup`, then
`HOME/.local/share/pangopup`; present invalid values fail rather than falling
through. One nonblocking root lock serializes installation. A marked private
stage receives each decompressed byte once while transport and reconstructed
hashes are checked. Files are synced and published with no-replace rename, a
canonical receipt binds the immutable directory, and `active.json` is replaced
atomically. Crash reconciliation removes only same-filesystem, effective-uid
owned, no-follow marked stages. Published bundles are never overwritten or
deleted. Status is lock-free apart from a nonblocking probe; lookup never takes
the install lock.

Reuse validates the receipt, bounded metadata hashes, member shapes and sizes,
and cheap `BundleOpen` structure without opening transport parts or hashing the
score payload. Complete semantic certification remains a build-time operation.

The future CLI and HTTP service will obtain the selected remote profile before
calling this installer. Remote sync must:

1. resolve the binary-pinned or explicitly requested asset set, never an
   unpinned “latest” release;
2. use the shipped local installation lock;
3. reuse a complete compatible local bundle without network access;
4. otherwise download missing archives to a temporary cache path;
5. pass the exact local transport directory to the shipped installer.

The future `pangopup assets sync` command will fetch one explicitly pinned
release manifest and then call the same installer. Offline mode will forbid
network access and name every missing or incompatible asset. Callers will still
be able to supply an already installed bundle path. Containers will be able to
bake the same verified bundle into an image or mount it read-only.

Current managed storage follows Linux XDG application-data conventions rather
than Pangolin's Python-package layout. macOS and Windows behavior remains
unimplemented. The core scoring library accepts already resolved paths and
performs no download or home-directory discovery.

On Linux, durable installed bundles live under
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`; transport archives and partial
downloads may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`. Installed data
are not cache: clearing a future download cache must not break a complete installation.

The explicit local installer does not require a network and is the primitive.
Remote sync only obtains the exact bytes named by a pinned manifest and then
calls that installer. A complete installed profile remains usable offline even
if the remote release is unavailable.

Transport and reconstructed hashes are mandatory in the single install stream.
Ordinary startup validates receipt/manifest identity, sizes, format versions,
and structures without rereading every byte. This keeps startup cheap and
avoids loading the whole mapped corpus merely to prove it has not changed.

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
