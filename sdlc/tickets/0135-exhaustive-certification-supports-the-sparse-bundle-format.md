---
flow: build
priority: 1
deps: ["0134"]
---
# Exhaustive certification supports the sparse bundle format

## Outcome

The asset certification boundary exhaustively verifies either supported SNV bundle format and returns one format-neutral certified member identity.

## What is true today

Ticket 0134 lets runtime lookup open fixed-v1 and sparse-direct-v1 bundles, but `certify_bundle_members_with_gene_limits` deliberately rejects sparse input. `CertifiedBundle` exposes fixed-only names and traversal. Asset packaging and installation therefore cannot authenticate a sparse release even though runtime lookup can read one.

## Scope

Make exhaustive certification dispatch by the already-admitted manifest format. Preserve fixed-v1 limits and results. Add the sparse 3 GiB ceiling from ADR 0027 before mapping or hashing, complete sparse verification, decoded logical digest and count validation, member hashing, notice validation, and format-neutral certified member access. Reader dispatch stays in `pangopup-index`; attribution, hashing, logical reconstruction, and manifest comparisons stay in `pangopup-assets`. Sparse traversal streams loci without a fixed-style gene buffer. The candidate builder retains an explicit fixed-only input contract. Update the index and runtime-data architecture documents. Do not build or activate a release profile, change the retained complete candidate, change score precision, or change public scoring output.

## Done, observably

- Fixed-v1 certification produces the same identity, decoded counts, errors, and bounded-gene behavior as before.
- Sparse certification checks both held and declared member sizes against the 3,221,225,472-byte ceiling before mapping or hashing, validates the exact notice and member hash, exhaustively decodes every ordinary and ambiguous locus, and matches both manifest logical digests and every reconstructable declared count. Ascending and descending source-member counts remain reconstructable only as their declared sum.
- Corrupt untouched sparse payload with a refreshed member hash, logical-digest drift, count drift, oversize declarations, and format/media/payload mismatch fail with stable typed errors before certification succeeds.
- `CertifiedBundle` reports its format, member size, member digest, and format-neutral exhaustive summary without exposing the wrong reader type.
- Miniature coverage includes exception-only genes, a mixed ordinary/ambiguous gene, coordinate gaps, and both supported omitted-base shapes.
- The sparse-candidate builder explicitly rejects a sparse source bundle without publishing output or leaving owned scratch paths.
- Miniature fixed and sparse certification tests, focused builder tests, `make lint`, `make test`, and `make spec` pass.

## Dependencies

Ticket 0134 supplies production bundle dispatch and strict manifest admission. Tickets 0130 through 0132 supply deterministic sparse writing, complete logical traversal, and the retained complete candidate.
