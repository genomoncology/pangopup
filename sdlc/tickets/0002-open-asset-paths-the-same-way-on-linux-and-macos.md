---
flow: build
priority: 6
---
# Keep asset authority descriptor-relative on Linux and macOS

## Outcome

PangoPup performs its local asset file and directory operations on Linux and macOS while preserving the authority of an already-open directory descriptor. No operation converts that descriptor back into a pathname and reopens through the name.

## Current facts

`pangopup-assets` rejects macOS before twenty asset operations run. The component validator already limits every opened name to one plain child component. Linux then adds descriptor-relative no-follow, same-device, directory-iteration, and no-replace guarantees. The macOS SDK provides descriptor-relative primitives for the same guarantees, but the repository does not use them.

The previous candidate reopened held directories through derived paths. A name replacement could redirect the operation after admission. That candidate was correctly refused.

## Done, observably

- Supported child-file opens, child-directory opens, directory iteration, and no-replace publication operate relative to a held directory descriptor on Linux and macOS.
- Replacing a directory name after its descriptor is admitted cannot redirect a later operation. A test replaces the name and requires refusal or continued use of the original descriptor on both platforms.
- A symlinked child is never followed on either platform.
- A child on a different device is refused by the same production decision on both platforms. A hermetic unprivileged test supplies mismatched device identity to that decision, so CI needs no mount privilege.
- Platform-specific code remains only where the operating systems expose different primitives. Each remaining split names that primitive in a code comment.
- Builder provenance stays truthful. Only artifact fingerprints whose declared source inventory changed may change, and a focused fixture proves that index content stays unchanged.
- `make lint`, `make test`, and `make spec` pass under the repository’s current Linux and macOS support matrices.

## Boundary

This ticket establishes the portable security boundary in the owning asset library. Ticket 0003 owns native macOS command support, macOS CI, user-facing documentation, and restored macOS specs.

Do not publish an asset or release. Do not weaken a Linux check. Do not change scoring, bundle verification, or uninstall behavior. Do not change a builder fingerprint whose declared source inventory did not change.
