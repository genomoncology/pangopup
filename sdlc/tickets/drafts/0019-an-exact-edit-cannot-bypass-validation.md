---
flow: build
priority: 5
---
# An exact edit cannot bypass validation

PangoPup provides fallible constructors for exact GRCh38 insertions and deletions, but the returned public value also exposes every field. Safe external Rust code can therefore construct states that the constructors reject. Conversion assumes those rejected states cannot exist. A deletion starting at base 1 can panic, invalid insertion geometry can be ignored, and other invalid deletion values can reach an assertion instead of a typed error.

Make construction the validation boundary. Safe callers must not be able to create an exact edit that violates the documented coordinate, sequence, or length rules without receiving an error. Conversion of every value that a caller can obtain through the public API must return a scoreable literal or a typed error and must never panic because an invariant was bypassed.

Keep the accepted edit forms and their current scoring behavior. The command-line and HTTP adapters must continue to accept every exact edit they accept today and return their existing client outcomes for invalid input.

Done, observably:

- External safe code cannot directly construct an exact edit that bypasses its fallible validation boundary.
- The public constructors still reject non-adjacent insertions, reversed or first-base deletions, sequence-length mismatches, empty or oversized sequences, and invalid DNA bases.
- Conversion has no caller-controlled subtraction or assertion that can panic for an exact-edit value supplied through the public API.
- Valid insertions and deletions produce the same literal variants, routing, cache keys, scores, and rejection outcomes as before.
- Command-line and HTTP behavior remains unchanged for valid and invalid exact-edit requests.
- `make lint`, `make test`, and `make spec` pass without reducing specification coverage.

Boundary: do not change the exact-edit wire syntax, accepted coordinates, allele limits, error codes, HTTP statuses, response shapes, scoring, routing, reference validation, caching, assets, or fingerprints. Do not change `pangopup-core`. Do not add serialization, configuration, or a new public edit form.
