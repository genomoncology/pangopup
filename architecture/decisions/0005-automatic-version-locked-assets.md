# 0005 — Automatic version-locked assets

Status: accepted
Date: 2026-07-21

Implementation status: Linux local installation, XDG discovery, canonical
receipts, active selection, and cheap reuse are shipped for caller-supplied
transport files. Pinned remote sync and automatic download remain future.

## Decision

The future remote-sync CLI and HTTP service will automatically obtain a binary-compatible
asset bundle when it is absent. The binary will pin an immutable manifest with
exact URLs, sizes, SHA-256 digests, format versions, source identities, and
licenses. It will never ask GitHub for an unpinned "latest" asset.

The shared asset adapter will use the platform's durable application-data
directory, not the cache directory, as the authority. On Linux this will be
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`. Downloads and partial archives
may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`.

The shipped local installer takes one nonblocking root lock, verifies transport
and reconstructed bytes in one stream, publishes an immutable receipt-bound
bundle, and atomically replaces `active.json`. Reuse and startup perform cheap
compatibility and structural checks. Complete semantic scanning stays in the
build-time verifier. Future remote sync and offline/container prefetch use the
same local installation boundary.

## Consequences

- A first run can become usable without manual data placement.
- Air-gapped and container deployments remain deterministic and network-free
  after preinstallation.
- The core scoring library remains deterministic and side-effect free: adapters
  pass it already resolved paths.
- A cache cleanup cannot remove authoritative installed data.
- Concurrent startup cannot expose a partial bundle.
