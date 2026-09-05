---
base: 74993b64ddb75391297197f5c84a75582da09e99
head: ba7e945ae8be6d7c7151fb67dab19fd573e025b0
---

# Accept exact GRCh38 indels

PangoPup now accepts strict exact GRCh38 insertion and deletion intervals through the CLI and HTTP service. It reads the required left anchor from the installed reference, verifies submitted deletion bases, and converts each edit to the existing canonical literal allele before routing, cache access, queue admission, or inference. Equivalent exact and anchored forms share output and persistent-cache identity. HTTP keeps valid neighboring results when one deletion sequence disagrees with the reference.

Design review moved the pre-canonical edit types and conversion into `pangopup-engine`. This preserved `pangopup-core`, source fingerprints, bundle manifests, and immutable asset identity. Code review found and corrected an explicit-path identity race. The CLI now binds and verifies the retained reference provider before conversion or cache access. Review also restored the README size ratchet, expanded focused help, and required successful deletion, boundary, and cache short-circuit proofs.

The accepted service holds one additional authenticated reference descriptor and memory map for request-time conversion. Model workers retain independent scorers. Runtime documentation records this resource cost.

Root verification passed `git diff --check`, `make lint`, `make test`, and `make spec`. The executable specification reported 281 passed and 7 skipped. Existing dependency-duplication warnings remained non-failing. The shared hardening issue remains because it contains held observations beyond this ticket.
