---
flow: build
priority: 4
---
# Name the first published container that carries the inventory

`architecture/compatibility.md` tells a consumer to deploy strict support for the response-shape inventory before deploying PangoPup v0.4.0. No v0.4.0 container exists. Publication stopped before container finalization, and fresh anonymous registry reads return `MANIFEST_UNKNOWN` for GHCR `0.4.0` and `v0.4.0`. The first published container carrying the inventory is v0.4.1.

A consumer that deploys the container reads a version it cannot pull. The document names the executable version that introduced the shapes and never says which image first shipped them. That consumer either searches for an absent image or assumes the inventory does not apply to the image it can pull.

The same sentence appears in `planning/artifacts/057-release-notes.md`. Those are the published, immutable v0.4.0 release-note bytes, pinned by exact SHA-256. Do not change them. Do not change the inventory heading, the inventory entries, the rejected-item shapes, the schema link, or the existing deployment-order sentence in either document.

Add the missing fact to the living compatibility document only.

Done, observably:

- `architecture/compatibility.md` states that PangoPup published no v0.4.0 container.
- It states that the first published container carrying this inventory is v0.4.1.
- The version gate requires both statements in that document, under the existing inventory heading.
- The gate applies that requirement to `architecture/compatibility.md` alone and not to the immutable release-note body.
- The portable version-consistency test holds an independent copy of the required text, asserts the gate matches it, and proves the gate rejects the document with the text removed.
- The pinned text names v0.4.1 as a literal. A later release does not move it, because the first container to carry the inventory does not change.
- `planning/artifacts/057-release-notes.md` is byte-identical to the published v0.4.0 release body.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: this is a documentation change. Do not change any behavior, field, value, limit, status, code, message, response shape, or version. Do not rewrite historical tickets, records, release notes, or publication evidence.
