---
base: b1e2560
head: 8811e6d
---

# Name the first published container that carries the inventory

`architecture/compatibility.md` now states that PangoPup published no v0.4.0 container and that the first published container carrying the response-shape inventory is v0.4.1. A consumer that deploys the container previously read a version it could not pull. Anonymous registry reads return `MANIFEST_UNKNOWN` for GHCR `0.4.0` and `v0.4.0`, and v0.4.0 shipped as an executable only.

The version gate requires both statements under the existing inventory heading. It applies that requirement to the living compatibility document alone. `planning/artifacts/057-release-notes.md` carries the published, immutable v0.4.0 release-note bytes and did not change; it still hashes to `729fa6ed9ddb641501f2abdf5e63cd2fd9861154a46f02967bea7ff408ce4aa9`, the value the gate pins against the public release body.

The pinned text names v0.4.1 as a literal rather than deriving it from the observed public container version. Which container first carried the inventory is a fixed historical fact and must not move when a later release ships.

The portable version-consistency test holds an independent copy of the required text, asserts that the gate matches it, and proves that the gate exits 1 when the text is removed from the document.

Three options were weighed. Adding the fact to the compatibility document alone was chosen. Leaving the document unchanged was rejected because the only current container consumer would search for an absent image. Renaming the inventory to v0.4.1 throughout was rejected because it would contradict the immutable published release body.

No behavior, field, value, limit, status, code, message, response shape, or version changed. `make lint`, `make test`, and `make spec` passed. The specification suite reported 283 passed and 7 skipped.
