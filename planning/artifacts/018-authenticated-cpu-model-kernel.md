# Ticket 018 — Authenticated CPU model kernel

Date: 2026-07-25

## Result

Pangopup now has a bounded raw Pangolin CPU kernel in Rust. Twelve exact
upstream checkpoints are converted into one ONNX graph, authenticated as a
closed three-file bundle, opened once through ONNX Runtime, and reused across
calls. The kernel accepts exact A/C/G/T/N contexts on either strand and returns
all twelve selected raw channels in genomic orientation.

This is not yet variant-level fallback. It deliberately excludes reference and
alternate context construction, ensemble arithmetic, masking, extrema,
lookup-miss routing, public score rendering, caching, asset delivery, and HTTP.

## Frozen inputs

- Upstream: `https://github.com/tkzeng/Pangolin`
- Commit: `5cf94b8db938c658391b4305cd7ce33297d44ff7`
- `pangolin/model.py`: 3,011 bytes,
  SHA-256 `4a1c5c2570aafe1452bb43332255321677e6c6c817adf84b9dd438e3ca4be6f8`
- Checkpoint set identity: `pangolin-1.0.2-5cf94b8-checkpoints-v1`

| Ordinal | Checkpoint | Bytes | SHA-256 |
|---:|---|---:|---|
| 1 | `final.1.0.3.v2` | 2,877,321 | `f0478fab173b75f7f7e9fe96688bad6c50fa4a46d70557f423b110caaf565501` |
| 2 | `final.2.0.3.v2` | 2,877,321 | `c4c6bb4880fa6fb28b14182ae3ea0600edb07056158f55325b5e6e6e48fc9f26` |
| 3 | `final.3.0.3.v2` | 2,877,321 | `ec685a6e7105a4486c1f89a005458a13deb3fe7171f13d434f4877e386d10676` |
| 4 | `final.1.2.3.v2` | 2,877,321 | `559c05de3e1ce65c2515ca3e92ef85edb0ec2e47686ca58060e25891ce06eb3a` |
| 5 | `final.2.2.3.v2` | 2,877,321 | `48758ba8b95eee9aa9feea52672ef06ca1b34111299c27f8a710f734d8b9aae5` |
| 6 | `final.3.2.3.v2` | 2,877,321 | `7cb576c2b24db4fdd6970c4ca4fb7c20ae1b1d8ae80645ebbe689848b5743129` |
| 7 | `final.1.4.3.v2` | 2,877,321 | `c50b12e0c0af776d5674ca5e346493f8265783494d4df383364de9c1136657f6` |
| 8 | `final.2.4.3.v2` | 2,877,321 | `e03303bed4fd6f135ec0f6c1b192cce954ea42d0646f44d17b4a6fbb2b1f610e` |
| 9 | `final.3.4.3.v2` | 2,877,321 | `9476d2e25520d7ff15bece0cd5d3b657e3b1dd3cc5fcab1d9c3b62bea7a0c5b6` |
| 10 | `final.1.6.3.v2` | 2,877,321 | `2aae563fa18a8a9b6699c6c96e0d32b8ec7543f8f805fb3bc9de77302cc9f66e` |
| 11 | `final.2.6.3.v2` | 2,877,321 | `7d3c0b1b2a60067b940dec315567874fbc8bcd322f1b7c76bf969f51f0f53f7f` |
| 12 | `final.3.6.3.v2` | 2,877,321 | `756e7721a382cace24e9bfea5b543af5623f2487d9a3efe7385e9c76367005fd` |

Raw checkpoint bytes total 34,527,852.

## Independent evidence

The maintainer environment is CPython 3.13.5, PyTorch 2.7.1+cpu, NumPy 2.5.1,
and ONNX 1.19.1:

- `tools/pangolin-model/uv.lock` SHA-256:
  `6d2f3ded757d1806e270ee72b6dc80190aee8a1c1bd295c90406cafbdcbba63d`
- evidence helper SHA-256:
  `cef204f0e706880fbfd29af8c0ec16bd0c2f7d0bfade7e8d28e1630212b633b6`
- converter helper SHA-256:
  `82e1e3bf38da2b65a0bfe0d711688f0f886e53f8c9c58424410ae1c50328f2b3`
- evidence manifest SHA-256:
  `9ce654730b76b34bbfdac826bec7c51c61ec50675b6abdb80f38a5a1ffeffaf2`
- tensor inventory SHA-256:
  `8a9a04c36c2497ea48ae7cf43e35d1a807a7c0a9827bacef6a218016d87314ed`
- kernel goldens SHA-256:
  `f9b026293ec8dbc8d87264ad1db38233f85dc4d6756aafca0929cd9894bf65aa`

The inventory contains 12 × 252 tensor records. Each checkpoint has 699,116
elements: exactly 32 scalar `int64` batch-normalization counters and 220 `f32`
tensors. The independent helper executes checkpoints separately, outside the
combined exporter, and records selected-channel `f32` bits.

Final self-review hardened both helpers to open authenticated inputs with
`O_NOFOLLOW`, require a regular single-link file and exact size before
allocation, read at most the expected size plus one through the held
descriptor, and parse the retained authenticated corpus bytes without reopening
the pathname. This changed only helper and evidence-manifest identities; the
checked inventory and golden member bytes above are unchanged. The
coordinator's independent accepted evidence run is the byte-equality proof.

## Accepted production build

The coordinator generated the evidence once into
`/home/ian/workspace/data/pangopup-model-018/evidence`. Its exact three-member
set is byte-identical to `tests/fixtures/pangolin-model-v1/`. The coordinator
then performed the one accepted conversion into the absent no-replace path
`/home/ian/workspace/data/pangopup-model-018/bundle`:

- bundle identity:
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`
- `manifest.json`: 3,823 bytes, SHA-256
  `4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`
- `model.onnx`: 33,867,142 bytes, SHA-256
  `3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b`
- `NOTICE`: 648 bytes, SHA-256
  `fbba767913348642351d7e95b8589619a8bb4a7f3738c5ea6fe266c21434107f`
- directory member bytes: 33,871,613

The ONNX graph is 660,710 bytes smaller than the raw checkpoints and the bundle
is below the 36,000,000-byte acceptance bound. Its graph bytes are identical
to the developer's earlier scratch conversion even though final held-descriptor
hardening changed the authenticated helper, manifest, and notice identities.

Rust qualification executed 14 cases, 18 strand paths, 36 sequence evaluations,
432 channel arrays, and all 45,756 scalar comparisons. No value was absent,
non-finite, or outside `[0,1]`. Maximum absolute error against independent
PyTorch output was `5.364418029785156e-7`, below the `1e-5` limit. This is
numeric-tolerance evidence, not a bit-identity claim.

## CPU runtime and measurement

- Rust: 1.93.1 (`01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf`)
- Target: `x86_64-unknown-linux-gnu`
- Kernel: Linux 6.17.0-35-generic
- Host CPU: AMD Ryzen 7 5825U with Radeon Graphics, 8 cores / 16 threads
- `ort` / `ort-sys`: 2.0.0-rc.12
- ONNX Runtime: 1.24.2, default CPU execution provider
- Session: sequential, graph optimization `All`, intra/inter-op `1/1`
- Native archive:
  `https://cdn.pyke.io/0/pyke:ort-rs/ms@1.24.2/x86_64-unknown-linux-gnu.tar.lzma2`
- Native archive SHA-256:
  `acc1cba79c337594ead1d88ca72516147aa60054c84217b53399a31caa5ba671`

The exact accepted-bundle command was:

```text
taskset -c 0 env \
  PANGOPUP_MODEL_BUNDLE=/home/ian/workspace/data/pangopup-model-018/bundle \
  cargo test --locked --release -p pangopup-model --test measure \
  -- --ignored --nocapture
```

The harness opens one session, including authenticated member reads and the
required 10,001-base initialization probe. It then uses deterministic
repeating-`ACGTN`, plus-strand contexts, performs three warmups and twenty timed
calls per length, consumes every result, and reads Linux `VmHWM`.

| Measurement | Observed |
|---|---:|
| CPU affinity | `0` |
| single-session open | 3,202,463,033 ns |
| process maximum RSS | 123,776 KiB |
| 10,101 bases p50 / p95 | 2,334,544,116 / 2,797,753,804 ns |
| 10,200 bases p50 / p95 | 2,221,989,361 / 2,748,089,929 ns |

There is no timing threshold. This is a raw one-request CPU baseline, not a
variant-level, concurrent, HTTP, accelerator, or end-to-end claim. It also
explains the lookup-first design: a precomputed SNV hit avoids seconds of model
work.

## Normal-gate proof

Normal tests use the checked synthetic bundle and real ONNX Runtime inference.
They cover both context bounds and strands, encoding and output validation,
canonical manifests, channel order, graph metadata, missing/extra/corrupt/
linked/replaced members, exact production evidence structure, and semantically
rebound evidence. They do not invoke Python, PyTorch, external checkpoints,
production assets, or a network.

Focused developer results after review remediation:

```text
cargo test --locked -p pangopup-model
  7 unit passed; 8 integration passed; 1 maintainer measurement ignored

cargo test --locked -p pangopup-build --test model_bundle
  4 passed

mustmatch test spec/model-kernel.md
  12 passed; 2 skipped
```

Coordinator final gates after independent review:

```text
make lint
  passed: cargo fmt --check and locked workspace clippy with warnings denied
make test
  passed: complete locked workspace; retained production-mask and
  maintainer model-measure tests ignored by contract
make spec
  152 passed; 2 skipped
```

The corrected release binary then inspected and requalified the unchanged
accepted bundle. It reported the same bundle identity and again compared all
45,756 scalars with maximum absolute error `5.364418029785156e-7`. No evidence
or conversion command was rerun.

## Scope retained for later

Variant normalization and context construction, reference and mask provider
integration, ensemble/post-processing parity, lookup-first routing, caching,
model asset publication/installation, HTTP, concurrency tuning, accelerators,
and quantization remain future work.
