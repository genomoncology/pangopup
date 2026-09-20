---
flow: build
priority: 1
deps: ["0140"]
---
# Publish the qualified v2 scoring assets

## Outcome

The exact retained `snv-grch38-v2` and `runtime-grch38-v2` publication sets are immutable public GitHub releases, and fresh anonymous downloads match every reviewed byte before production selects them.

## Done, observably

- Re-authenticate the expected GitHub account and prove both release tags are absent before the first write. Record Ian's 2026-09-20 authorization to publish PangoPup 0.5 and its scoring assets without storing a credential.
- Re-admit the exact retained release directories from tickets 0137 and 0140. Verify their complete inventories, canonical metadata, target and tooling provenance, sizes, and SHA-256 values against the checked authorities before upload.
- Create each release as a private draft, upload only its admitted inventory, compare the remote draft byte inventory, then publish once. Never replace, delete, or mutate a public asset.
- Confirm each public tag, title, target commit, release body, and asset inventory through a fresh anonymous read. Download every asset anonymously and compare its size and SHA-256 with the retained set.
- Prove the existing `snv-grch38-v1` and `runtime-grch38-v1` releases remain byte-identical and public.
- Add a durable publication record containing release identifiers, publication times, exact inventories, checksums, commands, and cleanup. Update the frontier. Repository gates pass from the record commit.

## Boundary

Publish the two qualified data releases only. Do not switch production selectors, change source code, tag v0.5.0, publish executable or container artifacts, alter scores, rebuild retained data, or delete v1.
