---
base: 037b31c1e8fcc65899e01c39bf6af7bdbbeae8a7
head: 0b1f3a293c3bd4d9653572ba507b32c6b38da03c
---

# Run asset-backed splice prediction natively on macOS

PangoPup now installs, synchronizes, inspects, and serves the asset-backed splice predictor on macOS. The same checked fixtures produce the same observable results on Linux and macOS. Continuous integration runs the portable macOS path, the complete Linux gates, and AMD64 and ARM64 container smoke checks.

The work also corrected portable file metadata and timeout assumptions. Transport reuse now validates compressed-part type, filesystem, and size without opening the parts. A fresh install still opens and authenticates every compressed byte before publication. A normal Linux user can therefore reuse a valid installed bundle when the original compressed inputs are no longer readable.

Several hosted Linux runs exposed distinct test and reporting defects after local macOS and root-container checks passed. The final diagnostic preserves the original gate status and publishes the final 1,300 raw log bytes within GitHub's observed 4,096-character annotation limit. Independent review accepted the diagnostic and metadata-only reuse changes after causal remediation tests.

Root verification passed `make lint`, `make test`, `make spec`, and `git diff --check`. A network-disabled Ubuntu 24.04 AMD64 run under uid 1000 passed the exact mode-`000` reuse test and the corrupt-fresh-install refusal test. Hosted CI run 33866509384 passed its macOS and Linux jobs. Hosted container run 33866509387 passed Ubuntu 24.04 smoke checks on AMD64 and ARM64. The production-model job remained intentionally skipped because the hosted run had no production assets.
