# 018 — Authenticate and execute the Pangolin CPU model kernel

Status: complete

## Why

Pangopup has exact SNV lookup, production reference and mask providers, and a
frozen upstream compatibility corpus, but it still cannot execute a Pangolin
model. The twelve exact upstream checkpoints are present locally and pinned by
the accepted compatibility profile. They are PyTorch checkpoint containers,
not a safe or useful Rust runtime format.

Establish one small model boundary before adding variant routing. Convert the
twelve authenticated checkpoints into one combined ONNX graph, load it through
a pinned CPU-only ONNX Runtime from Rust, and return the twelve selected raw
score channels in their exact checkpoint order. Keep ensemble arithmetic,
reference/alternate reconciliation, masking, extrema, public score rendering,
and request routing out of this ticket.

This is the first CPU baseline. It must prove numeric compatibility and retain
honest size, startup, memory, and inference measurements before Pangopup
considers thread tuning, another runtime, MPS, CUDA, or quantization.

## Scope

### Rust model boundary

- Add `crates/pangopup-model` to the workspace. It owns:
  - strict three-file model-bundle inspection;
  - one pinned ONNX Runtime CPU session;
  - bounded A/C/G/T/N context encoding for plus and minus strands; and
  - inference returning all twelve selected raw channels in genomic
    orientation.
- Expose typed bundle identity, strand, context, output-shape, and error values.
  The first kernel is deliberately single-owner:
  `infer(&mut self, context, strand)`. It owns one initialized session reused
  across calls and makes no `Sync`, pool, mutex, or concurrency claim. Do not
  add model-runtime types to `pangopup-core`; this is not yet the public
  scoring provider.
- Accept ASCII A/C/G/T/N case-insensitively. Encode `N` as four zeroes, reverse
  complement minus-strand input, and reject every other symbol with a typed
  position-bearing error. Accept context lengths `10_001..=10_200` only.
- The graph contract is:
  - input `sequence`: `f32 [1, 4, N]`;
  - output `replicate_scores`: `f32 [1, 12, score_length]`;
  - fixed batch and channel metadata (`1` and `12`) with dynamic length axes;
  - runtime `score_length == N - 10_000` on every call;
  - channels ordered by the twelve checkpoint ordinals in
    `tests/fixtures/pangolin-compat-v1/manifest.json`;
  - minus-strand output reversed back to genomic order before return; and
  - every value finite and in `[0, 1]`.
- Pin `ort = "=2.0.0-rc.12"` with default features disabled and only the
  features needed for `std`, `ndarray`, API 24, checksum-verified binary
  download, and one Rustls TLS implementation. This wraps ONNX Runtime 1.24.2.
  Use only its default CPU execution provider, sequential execution, one
  combined session, graph optimization level `All`, and intra-op/inter-op
  thread counts `1/1`.
- `ort` clean builds may fetch a target-specific native archive. For
  `x86_64-unknown-linux-gnu`, record the declared ONNX Runtime 1.24.2 archive
  URL and SHA-256
  `acc1cba79c337594ead1d88ca72516147aa60054c84217b53399a31caa5ba671`.
  Cargo/ORT dependency provisioning is separate from execution: once
  dependencies are present, tests and Pangopup runtime make no network
  request. Retain required ONNX Runtime notices.

### Minimal runtime bundle

- Define private `pangopup-model-bundle-v1` as one bounded directory containing
  exactly:
  - canonical closed `manifest.json`;
  - `model.onnx`; and
  - `NOTICE`.
- Define bundle identity as `sha256:` plus SHA-256 of the exact canonical
  manifest bytes. The manifest does not contain its own digest. It binds:
  - bundle kind (`production` or `synthetic-test`);
  - profile;
  - upstream/conversion source identity;
  - ordered source checkpoint filenames, sizes, and SHA-256 values for a
    production bundle;
  - checked converter, checkpoint-inventory, and qualification-evidence
    identities;
  - exact Python/PyTorch/NumPy/ONNX versions, opset 17, graph input/output,
    fixed checkpoint-to-channel mapping, and exporter settings; and
  - exact byte length and SHA-256 for every non-manifest member.
- Do not bind the Rust crate, ONNX Runtime, CPU, thread, or optimization
  identity into the model artifact. Those facts belong in qualification
  evidence and the later installed compatibility profile, so a runtime update
  does not create a false new model.
- Bound inputs before allocation: `manifest.json <= 256 KiB`,
  `NOTICE <= 64 KiB`, `model.onnx <= 48 MiB`, and aggregate bytes `<= 49 MiB`.
  Require the exact member set, regular single-link files, no followed
  symlinks, canonical manifest bytes, exact sizes, and exact digests.
- Load ONNX Runtime from authenticated bytes read through the held model-file
  descriptor, never by reopening a pathname after validation. Reject wrong
  graph names, element types, ranks, fixed dimensions, or dynamic-axis shape
  before returning a kernel. Initialization runs the minimum accepted
  10,001-base zero-context probe and requires `[1, 12, 1]`; every later call
  independently checks `[1, 12, N - 10_000]`.
- The checked miniature uses the same schema and graph contract but declares
  `kind=synthetic-test`, profile `pangopup-model-kernel-mini-v1`, synthetic
  source identity, and no production checkpoints. Generic inspection may open
  either kind. Production conversion and production qualification require the
  exact production kind/profile/checkpoint identities and cannot accept the
  miniature.

### Independent conversion and qualification evidence

- Add a locked maintainer-only uv project under
  `tools/pangolin-model/` with `.python-version`, `pyproject.toml`, and
  `uv.lock`. Pin CPython 3.13.5, PyTorch 2.7.1+cpu from the official CPU wheel
  index, NumPy 2.5.1, and ONNX 1.19.1. Python ONNX Runtime is not required.
- Add two separately authenticated helpers and Rust-owned adapters:
  - the **evidence helper** reads the exact upstream model source from
    authenticated bytes and independently executes each checkpoint without a
    combined wrapper. It produces checked tensor inventory and per-channel
    PyTorch goldens;
  - the **converter helper** exports the combined ONNX graph. It cannot derive
    its expected channel mapping or tensor inventory from its own output.
- Both helpers read every checkpoint into bounded bytes, verify exact
  filename/length/SHA-256 before parsing, and call `torch.load` only on the
  authenticated in-memory bytes with `weights_only=True` and CPU mapping.
  Authenticate upstream `model.py` before executing its exact bytes; do not
  import an unauthenticated pathname and hash it afterward.
- Check in the independent trust root at
  `tests/fixtures/pangolin-model-v1/`:
  - a canonical evidence manifest binding its exact helper/source/checkpoint/
    compatibility-corpus identities and member digests;
  - `checkpoint-tensors.jsonl`; and
  - `kernel-golden.jsonl`.
- The inventory binds, in strict model-state order, every tensor's checkpoint
  ordinal, name, shape, dtype, element count, canonical little-endian byte
  count, and value SHA-256. It requires exactly 252 entries and 699,116
  elements per checkpoint, including exactly thirty-two `int64`
  batch-normalization counters; all other tensors are `f32`.
- The independent golden helper loads each model strictly, calls `eval()`, runs
  without gradients, and selects channels independently as:
  - checkpoint ordinals 1–3: channel 1;
  - ordinals 4–6: channel 4;
  - ordinals 7–9: channel 7; and
  - ordinals 10–12: channel 10.
  It emits canonical `f32` bit patterns for each retained context, strand,
  reference/alternate sequence, checkpoint ordinal, and score position.
- Qualification-only code may construct the alternate contexts already frozen
  by the 14 scored compatibility cases. That construction exists solely to
  reproduce the independent kernel oracle; it is not a runtime variant API,
  normalization policy, reference provider, or indel-scoring implementation.
- The converter separately uses strict state loading, `eval()`, and a
  no-gradient combined wrapper with exact checkpoint order
  `(tissue 0,2,4,6; replicate 1,2,3)` and channels
  `(1,1,1,4,4,4,7,7,7,10,10,10)`. Export with `dynamo=False`, opset 17,
  input `sequence`, output `replicate_scores`, and only axis 2 dynamic.
  Normalize and validate the output metadata so batch is fixed at one, channel
  count is fixed at twelve, and the two length axes remain symbolic.
- Add Rust-owned commands:
  - `pangopup-build model evidence` creates an absent evidence directory from
    the exact upstream/corpus inputs for maintainer review;
  - `pangopup-build model convert` validates the checked evidence, invokes the
    exact converter without a shell, validates the completed bundle through
    `pangopup-model`, and atomically publishes to an absent output directory;
  - `pangopup-build model inspect --bundle <DIR>` emits one canonical JSON
    summary; and
  - `pangopup-build model qualify --bundle <DIR> --evidence <DIR>` replays the
    checked goldens through the Rust CPU kernel and emits cases, strands,
    sequence evaluations, channel arrays, scalar comparisons, maximum absolute
    error, bundle identity, and exact Rust/ORT/CPU/thread policy.
- Every helper invocation has closed arguments, no shell, bounded stdout/
  stderr, explicit failure cleanup, and no-replace publication. Conversion
  identity covers the exact helper bytes and locked maintainer environment.

### One accepted production build

- Use exactly:

  ```text
  uv sync --project tools/pangolin-model --locked
  cargo build --locked --release -p pangopup-build
  target/release/pangopup-build model evidence \
    --upstream /home/ian/foss/Pangolin \
    --python tools/pangolin-model/.venv/bin/python \
    --corpus tests/fixtures/pangolin-compat-v1 \
    --output /home/ian/workspace/data/pangopup-model-018/evidence
  target/release/pangopup-build model convert \
    --upstream /home/ian/foss/Pangolin \
    --python tools/pangolin-model/.venv/bin/python \
    --evidence /home/ian/workspace/data/pangopup-model-018/evidence \
    --output /home/ian/workspace/data/pangopup-model-018/bundle
  target/release/pangopup-build model inspect \
    --bundle /home/ian/workspace/data/pangopup-model-018/bundle
  target/release/pangopup-build model qualify \
    --bundle /home/ian/workspace/data/pangopup-model-018/bundle \
    --evidence /home/ian/workspace/data/pangopup-model-018/evidence
  ```

- Before accepting the production output, prove the generated evidence
  directory is byte-identical to the checked
  `tests/fixtures/pangolin-model-v1/` trust root. Preserve the accepted
  no-replace bundle and evidence under
  `/home/ian/workspace/data/pangopup-model-018/`. All subsequent checks use
  only `inspect` and `qualify`; they do not rerun `evidence` or `convert`.
- No scratch graph under `/tmp` is an input. The reviewed converter creates the
  accepted bundle once from `/home/ian/foss/Pangolin` commit
  `5cf94b8db938c658391b4305cd7ce33297d44ff7`.

### Tests, evidence, and documentation

- Check in a genuinely small synthetic bundle and evidence under
  `tests/fixtures/pangolin-model-kernel-mini/`. Normal gates execute real ONNX
  Runtime open/inference and exercise structure, corruption, context encoding,
  both strands, output shape/range, inspection, and qualification without
  Python, PyTorch, external checkpoints, a network, or production assets.
- Retain `planning/artifacts/018-authenticated-cpu-model-kernel.md` with exact
  source, checkpoint, converter, evidence, bundle/member, Rust/ORT/native
  archive, and host identities. Record raw-checkpoint versus ONNX/bundle bytes,
  qualification counts/error, single-session open latency/maximum RSS, and
  warmed representative 10,101- and 10,200-base latency under an exact
  CPU-pinned release-build method.
- Numeric acceptance is finite in-range output and absolute error `<= 1e-5`
  against every independent PyTorch scalar. Record the observed maximum; do
  not claim bit identity.
- Add discriminating rebound mutation tests for checkpoint/channel order,
  tensor name/shape/dtype/digest, graph I/O, one golden bit, NaN/Inf/range,
  minus-strand order, truncated/extra JSONL, and missing/extra/symlinked/
  replaced runtime members.
- Add `spec/model-kernel.md` for miniature inspect/qualify summaries, closed
  grammar, missing bundle, structural corruption, and a semantically rebound
  golden mutation.
- Add ADR
  `architecture/decisions/0014-authenticated-onnx-cpu-kernel.md`.
- Update `AGENTS.md`, `README.md`, `NOTICE`, `architecture/README.md`,
  `architecture/design.md`, `architecture/runtime-data.md`,
  `planning/faq.md`, and `planning/frontier.md`. Describe the raw CPU kernel as
  shipped while keeping variant-level fallback and model delivery future work.

### Explicit exclusions

- No public genomic/non-SNV request type, normalization, reference validation/
  windows, gene lookup, or mask-provider integration.
- No runtime reference/alternate subtraction, replicate averaging, tissue
  extrema, indel reconciliation, masking, score positions, rounding, or public
  score rendering.
- No lookup-miss routing, authoritative SNV-path change, result cache, HTTP,
  or end-user model command.
- No MPS, CUDA, XNNPACK, alternate runtime, quantization, reduced precision,
  configurable threads, session pool, or concurrency claim.
- No model installation, transport, upload, publication, production ONNX in
  git, or compatible four-asset profile. This ticket has no public external
  effect.
- No production SNV/reference/mask asset read, hash, or rebuild.

## Success Checklist

- The new crate and exact CPU dependencies are present:

  ```text
  cargo metadata --locked --format-version 1 \
    > target/ticket018-cargo-metadata.json
  jq -e '
    [.packages[] | select(.name == "pangopup-model")] | length == 1
  ' target/ticket018-cargo-metadata.json > /dev/null
  cargo tree --locked -p pangopup-model --prefix none \
    > target/ticket018-cargo-tree.txt
  rg -n '^ort v2\\.0\\.0-rc\\.12$|^ort-sys v2\\.0\\.0-rc\\.12$' \
    target/ticket018-cargo-tree.txt
  ```

- Normal locked tests run real CPU inference through the miniature and all
  named failure controls:

  ```text
  cargo test --locked -p pangopup-model
  cargo test --locked -p pangopup-build --test model_bundle
  mustmatch test spec/model-kernel.md
  ```

- Checked production evidence has twelve checkpoints × 252 tensor records and
  exactly 699,116 elements/32 int64 counters per checkpoint. Mutation tests
  prove these facts cannot be rebound together with an edited evidence
  manifest.
- The preserved production bundle contains exactly three files, is
  `<= 36_000_000` bytes, and `model.onnx` is smaller than the twelve raw
  checkpoints' combined `34_527_852` bytes.
- Production qualification executes 14 cases, 18 strand paths, 36 sequence
  evaluations, 432 sequence/channel arrays, and 45,756 scalar comparisons with
  no missing value. Every value is finite/in range and maximum absolute error
  is `<= 1e-5`.
- Production initialization proves the minimum context yields `[1,12,1]`;
  qualification additionally proves both accepted length bounds and every call
  checks `N-10_000`.
- The retained report records real CPU measurements without a flaky timing
  threshold or accelerator/concurrency/end-to-end claim.
- `git ls-files` contains no production ONNX/checkpoint or workspace-data path;
  the miniature is the only checked ONNX file.
- Current docs distinguish the raw CPU kernel from unimplemented variant-level
  scoring and asset delivery.
- `make lint`, `make test`, and `make spec` pass. Once Cargo/ORT/uv dependencies
  have been provisioned, these normal gates do not use network, Python,
  PyTorch, external checkpoints, or production model/reference/mask/SNV data.

## Decisions

1. **Runtime/representation:** choose one combined ONNX graph through pinned
   ONNX Runtime as the mature optimized CPU baseline. A native operator port or
   twelve Python/session boundaries add risk before a measured baseline exists.
2. **Runtime asset contents:** ship only manifest, model, and notice.
   Checkpoint inventories and goldens are source qualification evidence, not
   bytes needed to answer a request.
3. **Correctness authority:** authenticate whole checkpoint bytes before safe
   parsing, strict-load their checked independent tensor inventory, and compare
   the separately implemented exporter against separately generated
   per-checkpoint/channel PyTorch goldens.
4. **One graph:** emit all twelve channels from one session in fixed order.
   Keep channels separate so later Rust post-processing remains observable.
5. **Boundary:** raw tensor inference is separable from genomic-variant
   construction and Pangolin post-processing. Build the latter only after this
   kernel is exact and measured.
6. **Threads:** use `1/1` for the first reproducible baseline because the
   accepted PyTorch oracle did. Tune only from retained evidence later.
7. **Hermetic tests:** a tiny same-schema synthetic graph exercises real ORT in
   normal gates; the accepted external production bundle is built once and
   thereafter only reused.

## Dependencies

Tickets 009 and 017 complete. Ticket 009 supplies exact checkpoint/corpus
identity; Ticket 017 closed the last pre-model cleanup.

## Notes

- Base commit: `c30a72c7b1e00be70c7d4440f5dc609e618cc45a`.
- All twelve accepted `.v2` files are 2,877,321 bytes; total 34,527,852 bytes.
- A scratch feasibility graph was 33,869,089 bytes and observed maximum error
  `2.5331974e-7` under Python ONNX Runtime 1.23.2. That graph and number are not
  accepted Rust/ORT 1.24.2 evidence.
- The proven export shape used `sequence`/`replicate_scores`, opset 17,
  `dynamo=False`, and axis 2 dynamic. The reviewed converter deliberately
  normalizes the legacy exporter's symbolic output batch metadata to fixed one.
- No long-running “verify all” command or production rebuild becomes a normal
  gate. Production `qualify` is the meaningful retained all-channel check and
  runs only against the preserved model bundle.
- The coordinator records accepted production output and measurements before
  code review. Nothing is uploaded or published.

## Coordinator Authorship

Coordinator: Codex (`/root`)

Drafted from Ticket 017, ADR 0008, retained Ticket 009 evidence, the rolling
frontier, exact local checkpoint/ORT source inspection, and a read-only
combined-graph feasibility probe. The coordinator owns ticket remediation but
does not implement product code or approve its own ticket.

## Independent Ticket Review

Reviewer: Mencius the 3rd (`/root/ticket018_design_review`)

Accepted after one substantive remediation round. The initial review rejected
runtime/evidence coupling, circular conversion checks, a stronger graph shape
claim than the probe proved, incomplete dependency/environment pinning, and an
undefined mutability/qualification boundary. The coordinator reduced the
runtime asset to three files, separated and independently rooted qualification
evidence, fixed the exact checkpoint/channel and graph contracts, pinned
provisioning plus the native runtime archive, defined a mutable single-owner
kernel, limited alternate-context construction to qualification, and made the
accepted production output no-replace/reuse-only. The same reviewer rechecked
the complete amended contract against the local sources and accepted it with
no remaining findings.

Implementation then disproved one numeric prerequisite before publishing any
evidence: each checkpoint contains 32, not 16, scalar `int64`
`num_batches_tracked` tensors. The coordinator independently authenticated the
first checkpoint, reproduced 252 entries/699,116 elements/32 int64 counters,
and amended only that count in scope and acceptance. Material-amendment
re-review by the same reviewer accepted the corrected contract. The reviewer
confirmed that 16 residual blocks × two batch-normalization layers yields the
32 scalar counters and that no other scope changed.

## Implementation Evidence

Developer: Gauss (`/root/ticket018_implement`)

Developer evidence-generation result:

- purpose: generate the independently executed checked trust-root candidate;
- source: upstream commit `5cf94b8db938c658391b4305cd7ce33297d44ff7`
  and the exact frozen compatibility corpus;
- helper identity:
  `sha256:fb840e1c685eeb4317bdf2e7b9ea23e1b20642b4e6e81eb62267fdc1506dcf45`;
- locked environment identity:
  `sha256:6d2f3ded757d1806e270ee72b6dc80190aee8a1c1bd295c90406cafbdcbba63d`;
- command: `cargo run --locked -p pangopup-build --bin pangopup-build -- model evidence
  --upstream /home/ian/foss/Pangolin --python
  tools/pangolin-model/.venv/bin/python --corpus
  tests/fixtures/pangolin-compat-v1 --output
  /tmp/pangopup-model-evidence-018-dev`;
- result: stopped at the first authenticated checkpoint because the reviewed
  contract's counter count is false. `final.1.0.3.v2` has exactly 252 state
  entries and 699,116 elements, but 32 `int64` tensors named
  `resblocks.{0..15}.bn{1,2}.num_batches_tracked`, not 16. The remaining 220
  tensors are `f32`;
- control behavior: the helper rejected this mismatch before writing accepted
  evidence; the no-replace staging guard removed the partial directory;
- output: `/tmp/pangopup-model-evidence-018-dev`; no production path;
- blocker disposition: the coordinator independently reproduced the result and
  amended the success checklist, evidence schema contract, and mutation
  control to exactly 32 counters. The same independent ticket reviewer
  accepted that material correction, and implementation resumed.

Completed developer conversion and qualification checkpoint:

- purpose: prove the exact reviewed converter and Rust ONNX Runtime open/probe
  against the checked trust root without creating the coordinator's accepted
  production output;
- candidate: converter
  `sha256:8cfeab7c87096941b1748ea35314441a084b9ecc101cc0120d0633133bc02c8e`,
  evidence manifest
  `sha256:d441c023cbdd5495d0e58df60f034e974814907b81d6731597d4b491fff0d1e7`,
  upstream commit `5cf94b8db938c658391b4305cd7ce33297d44ff7`;
- command: `cargo run --locked -p pangopup-build --bin pangopup-build --
  model convert --upstream /home/ian/foss/Pangolin --python
  tools/pangolin-model/.venv/bin/python --evidence
  tests/fixtures/pangolin-model-v1 --output
  /tmp/pangopup-model-bundle-018-dev`;
- result: published an authenticated three-file scratch bundle after the Rust
  minimum-context probe. `model.onnx` is 33,867,142 bytes and SHA-256
  `3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b`;
  scratch bundle identity is
  `sha256:1dff4c9d142691006543f3590e009acc197c4c5b46e295d3a52871806ca6c2fc`;
- qualification: 14 cases, 18 strand paths, 36 sequence evaluations, 432
  channel arrays, and all 45,756 scalar comparisons; maximum absolute error
  `5.364418029785156e-7`, with no missing, non-finite, or out-of-range value;
- output: `/tmp/pangopup-model-bundle-018-dev`; failure cleanup is automatic,
  and this scratch output is not the accepted production path.

Implemented surfaces:

- new `pangopup-model` crate with the closed authenticated bundle, exact
  production checkpoint/channel contract, bounded context/strand encoding,
  descriptor-held ONNX load, graph checks, minimum probe, and raw
  twelve-channel CPU inference;
- locked independent PyTorch evidence and ONNX converter helpers plus
  Rust-owned `model evidence`, `convert`, `inspect`, and `qualify` adapters;
- checked complete production trust root and a tiny synthetic same-schema
  bundle/evidence fixture used by real ONNX Runtime tests;
- rebound and corruption controls covering channel/checkpoint/tensor/graph/
  golden/member identities and both strand/length bounds;
- executable miniature spec, ADR 0014, current architecture/product docs, and
  retained artifact report;
- an ignored maintainer-only release measurement harness. On CPU 0 of the AMD
  Ryzen 7 5825U host, one scratch session open was 2,763,246,509 ns with
  process `VmHWM` 123,684 KiB; repeating-`ACGTN` plus-strand p50/p95 were
  2,078,678,879/2,251,681,164 ns at 10,101 bases and
  2,451,126,316/3,249,250,519 ns at 10,200 bases over 3 warmups + 20 samples.
  There is no timing gate or wider performance claim.

Focused developer gates:

```text
cargo test --locked -p pangopup-model
  6 unit passed; 8 integration passed; 1 maintainer measurement ignored
cargo test --locked -p pangopup-build --test model_bundle
  4 passed
mustmatch test spec/model-kernel.md
  12 passed; 2 skipped
```

The developer did not read/hash/rebuild production SNV, reference, or mask
assets; did not create the coordinator-reserved accepted output; did not run a
second production conversion; and did not commit or push.

Final self-review found that the Python helpers authenticated input bytes after
`Path.read_bytes()` but did not enforce the ticket's pre-allocation bound or
retain the already authenticated corpus bytes. Both helpers now use
`O_NOFOLLOW`, require a regular single-link exact-size input before allocation,
read at most the expected bytes plus one from the held descriptor, and parse
the retained corpus bytes. Current identities are evidence helper
`sha256:cef204f0e706880fbfd29af8c0ec16bd0c2f7d0bfade7e8d28e1630212b633b6`,
converter
`sha256:82e1e3bf38da2b65a0bfe0d711688f0f886e53f8c9c58424410ae1c50328f2b3`,
and checked evidence manifest
`sha256:9ce654730b76b34bbfdac826bec7c51c61ec50675b6abdb80f38a5a1ffeffaf2`.
Inventory/golden bytes did not change. Per the no-needless-rebuild rule, the
developer did not rerun full evidence or conversion; the coordinator's one
accepted run must prove evidence byte equality and creates the first bundle
with these final helper identities.

Full developer handoff gates:

```text
make lint
  passed: cargo fmt --check and clippy --locked --workspace --all-targets
make test
  passed: complete locked workspace; retained production-mask and
  maintainer model-measure tests ignored by contract
make spec
  152 passed; 2 skipped
```

`cargo metadata --locked` contains exactly one `pangopup-model` package, and
the retained tree records `ort` and `ort-sys` exactly at `2.0.0-rc.12`.
`git diff --check` is clean. One initial direct focused spec invocation lacked
the Makefile's `target/debug` `PATH` and therefore produced command-not-found;
the corrected focused run and full `make spec` both passed.

After the final helper input-read hardening, the affected
`pangopup-build --test model_bundle` suite passed all four tests, the focused
model spec again passed 12 with 2 skips, both helpers passed CPython bytecode
compilation, and `git diff --check` remained clean. That change touches no
normal Python execution path; the earlier complete workspace gate remains
representative, and the coordinator will run the final full gate after
independent code review as required by the repository process.

The repository-root `NOTICE` remains byte-identical intentionally: it is the
embedded, hash-pinned notice in the already published immutable SNV bundle, and
changing it made the existing bundle fail its full test matrix. The new
three-file model bundle carries its own Pangolin GPL/conversion `NOTICE`.
ONNX Runtime is not contained in that model asset; complete dependency-license
packaging remains required when an executable/model release is actually
published.

First-pass adversarial review rejected two trust-boundary defects, both now
remediated without changing the reviewed ticket contract:

- Qualification now opens one `ModelKernel` and derives the evidence kind,
  profile, and emitted bundle identity from that held kernel. It no longer
  inspects the bundle path separately before opening the inference session. A
  deterministic regression replaces the complete bundle directory after the
  kernel opens and proves the qualification receipt names the held kernel's
  original identity rather than the different valid bundle now found at the
  same path.
- Runtime graph validation now requires the exact ONNX dimension-symbol
  contract as well as dtype and numeric shape: input `["", "", "N"]` and
  output `["", "", "N_minus_10000"]`. Unit controls reject a wrong or missing
  dynamic symbol and any symbol attached to a fixed batch or channel axis.

Focused remediation gates:

```text
cargo test --locked -p pangopup-model
  7 unit passed; 8 integration passed; 1 maintainer measurement ignored
cargo test --locked -p pangopup-build \
  model::tests::qualification_receipt_uses_held_kernel_identity_after_path_replacement
  1 passed
cargo test --locked -p pangopup-build --test model_bundle
  4 passed
PATH="$PWD/target/debug:$PATH" mustmatch test spec/model-kernel.md
  12 passed; 2 skipped
cargo clippy --locked -p pangopup-model -p pangopup-build \
  --all-targets -- -D warnings
  passed
git diff --check
  passed
```

These tests use only the checked miniature fixture. The accepted production
bundle and evidence were not read, altered, or regenerated; neither `model
evidence` nor `model convert` was run during remediation.

## Adversarial Code Review

Reviewer: `/root/ticket018_code_review`

The first review rejected the implementation with two medium findings:

1. qualification inspected a bundle and then reopened its pathname for the
   kernel, allowing a pathname replacement to make the receipt identify one
   bundle while inference used another; and
2. runtime graph validation checked numeric dynamic dimensions but did not
   enforce the reviewed ONNX symbolic-axis names.

The original developer remediated both without rebuilding evidence or the
model. Qualification now opens exactly one kernel, selects evidence and
sequences from that held kernel, and emits that kernel's profile and bundle
identity. A deterministic regression replaces the bundle directory with a
different valid bundle after open and proves qualification continues to execute
and report the original held session. Runtime graph validation now requires
exact symbols `["", "", "N"]` and
`["", "", "N_minus_10000"]`; negative controls cover wrong or missing
dynamic symbols and symbols attached to fixed axes.

The same reviewer independently reran the affected model/build tests, exact
path-replacement regression, clippy with warnings denied, and
`git diff --check`. Re-review accepted both remediations with no remaining
material finding, scope creep, or unjustified over-engineering. External
trusted bundle-ID binding remains intentionally deferred to delivery/profile
work.

## Accepted Production Evidence

Coordinator: Codex (`/root`)

- Ran the reviewed locked environment sync and release builder once, then
  generated evidence at
  `/home/ian/workspace/data/pangopup-model-018/evidence`.
- Proved the accepted evidence directory's exact three-member set byte-for-byte
  identical to `tests/fixtures/pangolin-model-v1/`. Its evidence identity is
  `sha256:9ce654730b76b34bbfdac826bec7c51c61ec50675b6abdb80f38a5a1ffeffaf2`.
- Performed the one accepted conversion into the absent no-replace path
  `/home/ian/workspace/data/pangopup-model-018/bundle`. Bundle identity is
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`.
  It contains exactly `manifest.json` (3,823 bytes), `model.onnx`
  (33,867,142 bytes), and `NOTICE` (648 bytes), totaling 33,871,613 member
  bytes. The model SHA-256 is
  `3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b`
  and is byte-identical to the developer's independent scratch conversion.
- Rust inspect and qualification passed 14 cases, 18 strand paths, 36 sequence
  evaluations, 432 channel arrays, and all 45,756 scalar comparisons. Maximum
  absolute error was `5.364418029785156e-7`.
- After code-review remediation, the corrected release binary inspected and
  requalified the unchanged accepted bundle with the same identity, counts,
  and maximum absolute error. No evidence or conversion command was rerun.
- Retained CPU0 release measurement against the accepted bundle: session open
  3,202,463,033 ns, `VmHWM` 123,776 KiB; 10,101-base p50/p95
  2,334,544,116/2,797,753,804 ns; 10,200-base p50/p95
  2,221,989,361/2,748,089,929 ns over three warmups and twenty samples.
- No production SNV, reference, or mask asset was read or rebuilt. No model or
  checkpoint was added to git, and nothing was uploaded or published.

## External Effect Evidence

Coordinator: not applicable; this ticket has no public external effect. The
accepted bundle remains local qualification evidence and is not a release
asset.

## Coordinator Final Check

Coordinator: Codex (`/root`)

After independent code-review acceptance:

```text
make lint
  passed: cargo fmt --all --check and
  cargo clippy --locked --workspace --all-targets -- -D warnings
make test
  passed: complete locked workspace; only the retained production-mask and
  maintainer-only model-measure tests were ignored by contract
make spec
  152 passed; 2 skipped
```

The exact Cargo metadata/tree checks found one `pangopup-model` package and
`ort` plus `ort-sys` exactly at `2.0.0-rc.12`. `git diff --check` passed. The
corrected release `model inspect` and `model qualify` commands reused the
preserved accepted output, reported bundle
`sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`,
and again passed 45,756 scalar comparisons with maximum absolute error
`5.364418029785156e-7`.

No production evidence/model regeneration, SNV/reference/mask access, upload,
publication, or public runtime routing occurred. The ticket outcome is
complete and ready for the reviewed implementation commit.
