---
flow: build
priority: 10
---
# Full uninstall removes an installed read-only profile

The public v0.4.0 executable installed and scored correctly against a disposable copy of the retained 15 GB profile. `pangopup uninstall --full --yes` then failed with `UNINSTALL_IO` while removing `receipt.json`. Normal installation deliberately makes admitted bundle and component directories mode `0555` and their files mode `0444`. Full uninstall tries to remove entries through those read-only directories without first establishing a safe writable removal boundary. Code-only uninstall succeeds and preserves the same profile.

Full uninstall must remove a valid managed installation with its normal read-only directory and file modes. It must retain the existing path, ownership, link, mount, replacement, and protected-root checks before changing permissions or deleting anything. Symlink roots must remain rejected. An admitted nested symlink entry must be unlinked without following or changing its target. A failure must not escape the admitted managed roots or weaken the executable-last behavior. Any managed directory that survives a failed traversal must retain or regain its admitted mode before it returns to its public path.

Done, observably:

- Full noninteractive uninstall removes the executable and normal read-only managed data and cache trees.
- A portable test installs or constructs the real managed permission shape and reproduces the former permission failure before the fix.
- Symlink roots, replacement, ownership, hard-link, mount-crossing, special-entry, nested-root, and protected-root cases remain rejected without deleting outside sentinels. Admitted nested symlink entries remain safely unlinked without following their targets.
- An injected failure after a read-only directory becomes writable leaves the executable installed and restores every surviving managed directory to its admitted mode before rollback exposes it.
- Code-only uninstall still removes only the executable and preserves data and cache.
- The normal lint, test, and executable specification gates pass.

Boundary: Do not change installation permissions, scoring behavior, asset identity, code-only uninstall, interactive confirmation, or JSON result shapes. Do not publish, delete, replace, or edit any public release, tag, executable asset, container alias, staged leaf, or scoring asset in this ticket.
