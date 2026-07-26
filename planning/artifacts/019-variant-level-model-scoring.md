# Ticket 019 — Variant-level model scoring evidence

Date: 2026-07-26

## Result

`pangopup-engine` now composes the shipped GRCh38 reference, GENCODE mask, and
mutable ONNX kernel into masked distance-50 scores for literal SNVs,
equal-length MNVs, and left-anchored insertions/deletions up to 100 bases. It
preserves exact reference/alternate context construction, fixed provider and
strand call order, upstream-compatible dtype/indel/ensemble arithmetic,
order-mutating masking, first extrema, and exact public hundredths.

This is a library boundary, not lookup fallback. It adds no CLI product output,
lookup routing, asset delivery, CPU tuning, batching, cache, pool, HTTP, or
long-running verifier.

## Checked normal evidence

Normal `pangopup-engine` tests consume:

- `tests/fixtures/pangolin-compat-v1/cases.jsonl`: 14 model cases, six
  rejection cases, and four controlled post-processing cases;
- `tests/fixtures/pangolin-model-v1/kernel-golden.jsonl`: 36 reference/
  alternate/strand evaluations and 432 channel arrays; and
- `tests/fixtures/pangolin-engine-v1/post-ensemble-sha256.tsv`: an exact
  3,026-byte receipt for the 18 typed strand-level loss/gain array pairs,
  SHA-256
  `3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b`.

The independent kernel goldens and the older batched compatibility capture
differ at low binary digits, consistent with the raw kernel's accepted
`1e-5` qualification tolerance. The new receipt binds exact arrays derived
from the checked kernel goldens; the frozen compatibility corpus independently
proves that all masked public hundredths, positions, warnings, and exact
plus/minus GENCODE order remain unchanged.

Focused normal results:

```text
cargo test --locked -p pangopup-core
  5 passed

cargo test --locked -p pangopup-index --test mask
  6 passed

cargo test --locked -p pangopup-index qualification_tests
  1 passed

cargo test --locked -p pangopup-engine
  7 unit passed; 1 production qualification ignored
```

The engine controls include exact contexts/call order, all frozen modeled and
rejection cases, the four post-processing cases, same-strand mutation,
empty-boundary warnings, deletion binary64 retention, first ties, signed-zero
collapse, positive/negative half-centi rounding in both dtypes, invalid output
sign/range, provider short-circuiting, and a selected extremum at `+149`. A
checked synthetic ONNX integration executes the concrete `ModelKernel` with
in-memory reference and mask providers.

## Causal builder provenance refresh

Ticket 019 widened the shared core `RelativePosition` for modeled indels while
restoring the narrower `-50..=50` invariant explicitly at every fixed-v1 SNV
ingestion, canonicalization, encoding, and decoding boundary. The causal
builder inventories intentionally include `pangopup-core/src/lib.rs` in both
artifact families and additionally include the SNV parser and codec in the SNV
family. Their future-build source identities therefore changed to:

- SNV:
  `b3bdc4d9d8e710fb554fd47f0cfc6f6a7bb764451069e6ae4a98534d8c5dc6a2`;
- reference:
  `8c94a75f3f30b9a9b72dadffb9f232dd2b28a0258f30feb69fac7703f529f23d`.

The repository-native bounded regression generator and miniature reference
builder produced manifest identities
`fbb637198f52a28f93c43bf6803cfe7cfcb2d13351b518025ef78a65373610b5`
and
`3c1ed047d7baf97eab11959a9bdb1a71b6434453620a50b682d8ee7aaacf0ab8`.
Byte comparison proved the SNV source excerpts, fixture reference, requests,
NOTICE, and `scores.pgi` stayed unchanged; the score member remains
`fb0a77425456bd39e6aab7ad3447a24757f6889e82f7b27df01c214b78f8a6b9`.
The reference NOTICE and `reference.pgr` likewise remain
`faea3b1976bf4e15f95bad3906144d83b4441f860d3c5b87ab406205e47262db`
and
`0ef815ffb3fbb897e880e56afcb57e1edb41f3707784f591c0457581c2e9a3d5`.
Only current miniature provenance manifests and bundle-ID-bearing expected
JSONL changed. Legacy manifests, production identities/assets, dependency and
root-wiring projections, and Ticket 013's historical evidence remain intact.

## Mask qualification boundary

Ordinary `MaskDomainsOpen::open` remains cheap. The separate qualification
open uses one no-follow regular single-link descriptor for bounded SHA-256,
structural validation, and mmap construction, verifies the pathname still
names that inode, retains the descriptor, and returns exact byte/digest
identity. Normal tests reject a wrong digest, mutation, symlink, and pathname
replacement. This qualification assumes ADR 0013's immutable-inode contract;
concurrent in-place mutation or truncation after verification is outside the
supported threat model.

## Retained production qualification

The coordinator ran the single ignored Rust-only qualification after the
normal tests. The harness opened, but did not rebuild, the accepted model,
reference, and mask assets and compared all 14 masked cases with the
compatibility oracle:

```text
cargo test --locked -p pangopup-engine --test production_qualification \
  -- --ignored --nocapture
```

Result: passed, 14 cases and 21 ordered gene records. The emitted receipt
authenticated model
`sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`,
reference
`sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
the 6,703,320-byte mask at SHA-256
`714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`,
and reported post-ensemble receipt SHA-256
`3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b`.
Normal tests independently authenticated that checked receipt. After code
review, the ignored harness itself was tightened to read at most 3,027 receipt
bytes, require the checked 3,026-byte length, hash those bytes directly, and
reject a digest mismatch instead of merely printing the constant. The
developer compiled but did not rerun that production-assets test.
The developer did not open production assets; the repository contract
reserved this small review output for the coordinator.

## Limits

The scorer has no gene filter because all containing genes must participate in
ordered masking before a future router filters. The current ONNX policy remains
single-thread `1/1`; no complete-request latency claim is made. The exact
reference, mask, and model assets remain local retained inputs without a
coherent transport/install/release profile.
