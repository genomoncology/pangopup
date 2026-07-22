# 0005 — Automatic version-locked assets

Status: accepted
Date: 2026-07-21

Implementation status: target design; no asset manager, automatic installer,
or XDG discovery is shipped yet. The current CLI requires an explicit bundle
path.

## Decision

The future CLI and HTTP service will automatically install a binary-compatible
asset bundle when it is absent. The binary will pin an immutable manifest with
exact URLs, sizes, SHA-256 digests, format versions, source identities, and
licenses. It will never ask GitHub for an unpinned "latest" asset.

The shared asset adapter will use the platform's durable application-data
directory, not the cache directory, as the authority. On Linux this will be
`${XDG_DATA_HOME:-$HOME/.local/share}/pangopup/`. Downloads and partial archives
may use `${XDG_CACHE_HOME:-$HOME/.cache}/pangopup/`.

Installation will take a lock, verify transport and extracted members, and
publish atomically. Reused startup will perform cheap compatibility and
structural checks. Full rehashing will be an explicit verification operation.
Offline mode and an explicit preinstall command will use the same manifest and
validation path.

## Consequences

- A first run can become usable without manual data placement.
- Air-gapped and container deployments remain deterministic and network-free
  after preinstallation.
- The core scoring library remains deterministic and side-effect free: adapters
  pass it already resolved paths.
- A cache cleanup cannot remove authoritative installed data.
- Concurrent startup cannot expose a partial bundle.
