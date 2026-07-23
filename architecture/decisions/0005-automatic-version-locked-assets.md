# 0005 — Automatic version-locked assets

Status: accepted
Date: 2026-07-21

Implementation status: Linux local installation, XDG discovery, canonical
receipts, active selection, cheap reuse, and explicit pinned remote sync are
shipped. Automatic service-start provisioning remains future.

## Decision

The shipped remote-sync CLI obtains a binary-compatible asset bundle when it is
absent. The binary pins an immutable manifest with
exact URLs, sizes, SHA-256 digests, format versions, source identities, and
licenses. It will never ask GitHub for an unpinned "latest" asset.

The shared asset adapter will use the platform's durable application-data
directory, not the cache directory, as the authority. On Linux this will be
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`. Downloads and partial archives
may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`.

The shipped sync adapter takes a separate nonblocking cache lock, streams the
five members sequentially, resumes only with an exact strong validator and
range, and atomically publishes a closed cache transport. The local installer
takes one nonblocking root lock, verifies transport
and reconstructed bytes in one stream, publishes an immutable receipt-bound
bundle, and atomically replaces `active.json`. Reuse and startup perform cheap
compatibility and structural checks. Complete semantic scanning stays in the
build-time verifier. Remote sync and offline prefetch use the same local
installation boundary; future containers may do the same.
Cache traversal is no-follow and descriptor-held; member publication and
eviction never depend on re-resolving an earlier checked pathname.
First-use profile and lock creation is race-idempotent, and the profile lock
precedes all partial/member/transport inspection or mutation. Directory
enumeration remains streaming even for hostile cache contents.

## Consequences

- A first run can become usable without manual data placement.
- Air-gapped and container deployments remain deterministic and network-free
  after preinstallation.
- The core scoring library remains deterministic and side-effect free: adapters
  pass it already resolved paths.
- A cache cleanup cannot remove authoritative installed data.
- Concurrent startup cannot expose a partial bundle.
