---
base: 0d1d9e883035d8142bd8af1aca96db079deb85b1
head: c385c9a7d376f7399dbf5f7b9260992369438383
---

# Every caller-visible service fact has one source

Status now reads the CPU policy from the same provenance used by scoring and identity. One caller-facing contig policy serves scoring, status, and the maintenance reference command while stored source identities remain strict. Request-limit refusals now render the values used by enforcement and status. Public values and behavior did not change.

Independent design and code reviews accepted the final change without findings. A source-fingerprint test rejected an initial placement inside the immutable reference-builder boundary, and the implementation moved the policy outside that boundary before review. Focused package, behavior, and fingerprint tests passed. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 282 passed and 7 skipped.
