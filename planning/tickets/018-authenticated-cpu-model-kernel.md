# 018 — Authenticate and execute the Pangolin CPU model kernel

Status: ready

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
  elements per checkpoint, including exactly sixteen `int64`
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
  exactly 699,116 elements/16 int64 counters per checkpoint. Mutation tests
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

## Implementation Evidence

Developer: pending

## Adversarial Code Review

Reviewer: pending

## External Effect Evidence

Coordinator: not applicable

## Coordinator Final Check

Coordinator: pending
