---
base: d363d5ea6968554738b9ede1c3aa1331e1071249
head: 247972bca5df42fd794db5158c450f50b89026b8
---

# Accept mitochondrial caller aliases

CLI, HTTP, and reference-window inputs now accept `MT` and `chrMT` and normalize them to canonical `chrM`. Source ingestion, stored manifests, the core contig parser, and RefSeq behavior remain unchanged.

The first implementation changed the fingerprinted core parser. The full gate caught the resulting hard source-fingerprint change. Remediation moved the aliases to user-input adapters and restored both asset fingerprints without changing their constants. The same reviewer accepted the correction. Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`; the executable specification reported 275 passed and 7 skipped.
