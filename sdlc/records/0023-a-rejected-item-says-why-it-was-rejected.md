---
base: cbad4c4
head: ea67ee457268281eeb13887e69ec5f204ac30764
---

# A rejected item says why it was rejected

Every rejected HTTP score item now carries a stable machine-readable reason. The published vocabulary distinguishes malformed input, unsupported genomic values, invalid exact-edit geometry, unsupported model shapes, alleles above the model limit, unavailable reference context, reference mismatch, unsupported reference symbols, positions outside annotated genes, and future model rejections.

The existing item status, error code, error message, ordering, scoring behavior, and command-line messages remain unchanged. Reason values describe conditions without exposing coordinates, sequences, symbols, offsets, paths, provider details, or blame.

Initial code review found that exact-edit conversion collapsed unavailable reference context and unsupported anchor symbols into invalid geometry. Remediation preserved those causes through conversion and added focused coverage. Independent re-review accepted the correction. Complete tests for both changed packages, Clippy with warnings denied, HTTP specifications, formatting, and `git diff --check` passed.
