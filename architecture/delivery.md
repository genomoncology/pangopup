# Artifact Delivery

This document records the shipped immutable SNV release, local transport,
Linux installation, and pinned resumable remote sync. The runtime opens either
an explicitly supplied bundle path or the active receipt-bound bundle in Linux
user data.

## Repository publication-security boundary

The ordinary repository gate includes a checked Rust dependency policy through
cargo-deny 0.19.4. It denies vulnerable, unsound, yanked, unapproved-license,
and unknown-source dependencies, carries no advisory ignore or license
exception, and reports unmaintained advisories and duplicate versions. The
workspace's versionless local path edges are preserved because they are
builder-causal manifest inputs; cargo-deny 0.19.4 cannot exempt those edges
while independently denying registry wildcards for publishable workspace
crates. The reviewed policy therefore allows wildcard linting, while the locked
graph and source policy admit only workspace paths and the canonical crates.io
registry.

CI has an explicit `contents: read` token, pins every action to a full commit,
and installs mustmatch, cargo-deny, and ripgrep through reviewed exact digests.
GitHub repository settings are an external coordinator-owned boundary rather
than runtime code. Before another public asset family, the live repository must
also verify read-only Actions defaults, disabled Actions PR approval, enabled
Dependabot security updates, the three writable requested secret-scanning
controls, and two active `main` rulesets: unbypassed
deletion/non-fast-forward protection plus pull-request/`gate` requirements with
only the reviewed administrator-role bypass. Validity checks have no reviewed
repository-local write and remain optional: Pangopup has no configured
repository secrets or open secret alerts, and provider validity is alert
triage rather than an asset-publication requirement.
Organization code-security configurations can set validity checks and attach
to selected repositories, but the existing recommended configuration enables
broader features. A custom configuration requires `write:org`, asynchronous
verification, and a rollback design that accounts for detach retaining applied
settings, so it is a separately reviewed authority expansion.

This security baseline itself published no runtime asset. Release-specific
attribution, stable derived bytes, GPL preferred source, controlled upload,
remote digest comparison, and immutable finalization are now complete for
`runtime-grch38-v1`. Runtime sync and clean-machine inference remain distinct
delivery steps.

## GitHub Releases

[GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
are the initial distribution channel. GitHub currently permits up to 1,000
assets on one release, requires each asset to be under 2 GiB, and states no
aggregate release-size or bandwidth quota. Ordinary Git objects remain subject
to the 100 MiB limit, so generated indices and model weights never enter Git
history or Git LFS.

Release families keep independently versioned concerns separate. The SNV
lookup uses one shipped eight-file release asset set whose installable transport
is the closed five-file subset. The executable remains a future public family.
The converted model, compiled GRCh38 sequence index, and compiled mask form one
qualified compatibility-bound public immutable `runtime-grch38-v1` release.
Their model-side local transport is shipped:

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
Model-side local transport (exactly ten files):
  runtime-transport.json
  runtime-profile.json
  model-manifest.json
  model-NOTICE
  model.onnx.zst
  reference-manifest.json
  reference-NOTICE
  reference.pgr.zst
  mask-NOTICE
  domains.pgm.zst
Model-side publication metadata:
  runtime-release-profile.json
  SHA256SUMS
  RELEASE-NOTES.md (release body; not uploaded)
Model-side preferred source:
  pangolin-5cf94b8-source.tar.zst
  pangopup-e6d8497-source.tar.zst
  Pangolin-GPL-3.0.txt
```

The twelve runtime upload members contain only Pangopup's derived runtime
members and metadata. The raw
Zenodo archive and extracted TSVs, NCBI FASTA and assembly report, GENCODE GTF
and SQLite database are upstream inputs or maintainer evidence and are never
mirrored as Pangopup runtime release assets. The complete exact upstream
Pangolin tree, including the original checkpoint containers, accompanies the
converted model as GPL preferred source; it is not installed runtime data. The
exact Pangopup target tree separately carries the converter and build source.

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
No Pangopup crate or binary uploads release assets. A later independently
reviewed publication lifecycle will have the coordinator invoke an
authenticated official `gh` executable directly. Deterministic preparation
does not freeze a local pathname: that lifecycle must define a controlled
stable source, upload the exact closed inventory to a non-public mutable draft,
compare GitHub's names, sizes, digests, and target commit while correction is
still possible, and only then publish/finalize and verify immutability (or
prove an equivalently safe order supported by GitHub). Publication credentials
and public mutation stay outside the runtime architecture.

The shipped local model-side transport carries the authenticated model bundle,
compact-reference bundle, selected mask member and checked GENCODE notice, and
a byte-exact runtime profile. Each large member has its own deterministic
Zstandard frame; one canonical manifest prevents a mixed tuple. Original
checkpoint containers and qualification goldens remain maintainer evidence.
Lookup-only installations can continue to avoid all three model-side frames.

```text
pangopup-build runtime-transport pack --profile <PROFILE> --model-bundle <MODEL_BUNDLE> --reference-bundle <REFERENCE_BUNDLE> --mask <DOMAINS_PGM> --output <ABSENT_DIR>
pangopup-build runtime-transport verify --transport <TRANSPORT_DIR>
pangopup-build runtime-transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
pangopup-build runtime-release prepare --transport <TRANSPORT_DIR> --target-commit <40_LOWERCASE_HEX> --output <ABSENT_DIR>
```

Pack authenticates component/profile consistency but does not promote a
profile to the trusted production tuple. Verify streams stored and
reconstructed identities without writing payloads. Unpack performs each decode
once into private same-filesystem staging and atomically publishes the
reconstructed layout. None of these commands opens or packages the SNV member.

`runtime-release prepare` is a separate production-only admission boundary. It
streams and authenticates stored and decompressed frame identities without
materializing decoded payloads, then copies its exact ten stored members once
through the same held no-follow descriptors and revalidates the closed source
inventory, and creates a private read-only thirteen-file publication stage.
The canonical release profile fixes the release identity, target commit,
runtime/component identities, twelve preferred upstream checkpoint sources,
converter paths, and exact member URLs, sizes, and digests. `SHA256SUMS` covers
the ten upload members plus that profile; release notes remain local input for
the GitHub release body. The command performs no network operation, rebuild,
recompression, decoded output, tag creation, or upload.

Ticket 034 adds no product uploader. Its retained supplement contains
deterministic `git archive` plus Zstandard representations of the complete
upstream Pangolin and target Pangopup trees and a byte-identical standalone GPL
license. Together with the Ticket 033 stage, the publication input is a closed
15-asset, 859,643,413-byte set. Only the coordinator may pass those exact
read-only paths to official `gh`, after the publication-ready commit's remote
gate and a fresh source/inventory audit. Publication starts private, verifies
the complete remote name/size/digest set after every one-time upload, and
becomes public only once; afterward `immutable=true` is mandatory.

That lifecycle completed as release
[`runtime-grch38-v1`](https://github.com/genomoncology/pangopup/releases/tag/runtime-grch38-v1).
Its 15 assets total 859,643,413 bytes, every upload was attempted once, and
GitHub's size/digest/state matched after each upload. The completed release is
immutable and its lightweight tag points directly to the pinned target commit.

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

All four SNV delivery stages are shipped. Deterministic local packaging and
publication of the converted model, GRCh38 sequence index, and mask are also
shipped. Pinned download and install-from-transport integration remain
separate reviewed outcomes.

Retained evidence selected the `acgt2-rle-v1` reference payload by speed from
three exact mmap candidates on the pinned six-contig RefSeq input. It now feeds
a separate production `PGRREF01` bundle and provider. The discarded candidate
files, miniature, benchmark executable, and CLI have been removed; their
reports and decisions remain historical evidence rather than runtime assets.
Compiled GRCh38 sequence index XDG installation, local transport, and
publication are shipped. Automatic remote provisioning remains future work.
The raw FASTA and assembly report are never packaged.

Ticket 012's canonical export, unselected candidates, phase receipts, and
benchmark report are retained private historical evidence and must not be
uploaded as runtime assets. Their one-time writer, qualification binary, and
source interfaces are no longer present at HEAD. ADR 0013 permits only the
exact selected `domains.pgm` member to become the runtime asset. Its
manifest-bound local transport verification, XDG installation, and publication
are shipped; automatic remote provisioning remains separate work. The production
reader performs none of those delivery operations.

## Shipped Linux installation and pinned remote sync

The binary embeds the canonical `snv-grch38-v1` release profile for one
compatible lookup asset set. The shipped full runtime profile separately
identifies the converted model, compiled GRCh38 sequence index, and mask
assets; its offline XDG installation and activation path is established.
The exact public payload is immutable and public. Automatic remote
provisioning and direct install-from-runtime-transport remain future work.

The shipped local command accepts an already available transport:

```text
pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
pangopup assets status [--data-dir <ABSOLUTE_PATH>]
pangopup assets sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
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

The shipped explicit CLI obtains the selected transport before calling this
installer. Remote sync:

1. uses only the binary-pinned asset set, never an unpinned “latest” release;
2. holds a separate nonblocking cache lock before the installation lock;
3. reuses a complete compatible local bundle without network access;
4. otherwise stream missing members sequentially to private cache state,
   resuming only on an exact strong-ETag ranged response;
5. pass the exact local transport directory to the shipped installer.

Cache traversal and mutation stay behind held Linux directory descriptors:
`openat2` rejects symlinked components, `mkdirat` creates private directories
as mode 0700, and member publication and eviction are descriptor-relative.
Bootstrap/profile/lock creation tolerates simultaneous first use; the profile
lock is acquired before working directories or a published transport are
inspected or changed. Hostile directory entries are validated and removed as a
bounded stream rather than collected in memory.

`pangopup assets sync` uses the compiled canonical profile directly; it does
not fetch a profile, query the GitHub API, or accept an arbitrary URL. Its
bounded rustls client accepts only the reviewed GitHub and release-download
hosts over HTTPS and follows at most five explicit redirects. Offline mode
forbids network access and names every incomplete member. Callers can still
supply an already available transport. Future containers can bake the same
verified bundle into an image or mount it read-only.

Current managed storage follows Linux XDG application-data conventions rather
than Pangolin's Python-package layout. macOS and Windows behavior remains
unimplemented. The core scoring library accepts already resolved paths and
performs no download or home-directory discovery.

On Linux, durable installed bundles live under
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`; transport archives and partial
downloads use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`, with
`PANGOPUP_CACHE_DIR` and `--cache-dir` overrides. Installed data are not cache:
clearing the download cache does not break a complete installation.

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
installs fail closed. Converted model, compiled GRCh38 sequence index, and mask
assets remain separately versioned.

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

The immutable converted-model, GRCh38 sequence-index, and mask publication is
complete. Pinned model-side sync and clean-machine compatibility-corpus
inference remain the next delivery proof. Executable/container dependency
inventory, SBOM, provenance, signing, and broader rollback policy remain later
release-hardening outcomes, not shipped claims.
