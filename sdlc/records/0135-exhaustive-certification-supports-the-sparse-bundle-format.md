# Exhaustive certification supports the sparse bundle format

Asset certification now dispatches by the admitted SNV format. Fixed-v1 keeps its prior gene bounds, decoded results, identities, and errors. Sparse-direct-v1 applies its 3,221,225,472-byte ceiling before mapping or hashing, streams every logical locus, checks reconstructable counts and logical digests, hashes the held member, and returns the same format-neutral certification facts.

The retained candidate builder still accepts only fixed-v1 input and rejects sparse input before it creates output or scratch data. Miniatures cover exception-only and mixed genes, gaps, both omitted-base shapes, refreshed-hash corruption, count drift, size limits, and format, media, and payload disagreement.

Independent design review accepted the ticket with explicit allocation, ownership, fixture, and error-preservation guardrails. Independent code review rejected one fixed-v1 declared-size error regression. The remediation restored `BUNDLE_INVALID` for that fixed case while retaining early `BUNDLE_INDEX` sparse rejection, and re-review accepted it. `make lint`, `make test`, `make spec`, and `git diff --check` passed on macOS. The specification gate ran 193 blocks.
