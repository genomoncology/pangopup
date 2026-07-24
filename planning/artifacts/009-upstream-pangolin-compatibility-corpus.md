# Ticket 009 — Upstream Pangolin compatibility corpus evidence

Date: 2026-07-23

## Outcome

Pangopup retains `tests/fixtures/pangolin-compat-v1`, a 227,060-byte strict
offline oracle for profile `pangolin-1.0.2-5cf94b8-grch38-v1`. It contains 24
ordered cases: 14 scored genomic cases, six rejection cases, and four
controlled post-processing cases covering all 28 required cells. Normal tests
replay its raw arrays and behavior in Rust; they do not run the model.

Final member identities:

| Member | Bytes | SHA-256 |
|---|---:|---|
| `manifest.json` | 5,337 | `fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b` |
| `cases.jsonl` | 220,071 | `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8` |
| `NOTICE` | 1,652 | `edb9addea955d89820b82cc77c86b2e879f843081dcd57b0940dcefe1698d5da` |

The final manifest is the corpus identity. `cases.jsonl` has exactly 24 compact
lines. M01–M11 retain `f32` model arrays; deletion cases M12–M14 retain
upstream-promoted `f64` arrays without narrowing. P04 fixes eight binary32 and
four binary64 rounding/rendering controls, including signed zero.

## Capture provenance

- Pangolin source: commit
  `5cf94b8db938c658391b4305cd7ce33297d44ff7`, declared version 1.0.2,
  tracked source blobs checked against the supplied clean worktree.
- Models: the twelve ordered `final.<replicate>.<tissue>.3.v2` checkpoints
  listed in the manifest, each 2,877,321 bytes and checked against its accepted
  SHA-256 before inference.
- Reference source: NCBI RefSeq `GCF_000001405.40_GRCh38.p14` gzip,
  972,898,531 bytes, SHA-256
  `11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3`;
  assembly report 80,454 bytes, SHA-256
  `64318ddff470b69b261a667d813210044f60d4ce654253a547db80ff73638d38`.
- Capture reference: the deterministic uppercase chr-named six-contig FASTA,
  671,294,255 bytes, SHA-256
  `81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb`.
- Annotation database: upstream GENCODE v38 SQLite, 380,366,848 bytes,
  SHA-256 `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6`.
- Annotation GTF: 46,556,621 bytes, SHA-256
  `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050`,
  official MD5 `16fcae8ca8e488cd8056cf317d963407`.
- Environment: CPython 3.13.5, PyTorch 2.7.1+cpu, NumPy 2.5.1, pandas
  3.0.3, pyfastx 2.3.1, gffutils 0.14, PyVCF3 1.0.4, Linux x86_64, and CUDA
  disabled. The normative helper explicitly used PyTorch intra/inter-op `1/1`.
  The auxiliary unmodified CLI used `OMP_NUM_THREADS=1`, which produced
  PyTorch intra-op `1` and the observed fresh-process inter-op default `16`.

The accepted typed-array candidate used implementation-state SHA-256
`9b22022a2be17fee8e3a4eabd387f0634af544dd164d174b7c4aac3bc58ee97c`,
binary SHA-256 `28a8b896192b9a387f13cba1cf188b3b0a7066d2f0f4b701dd9f6da34226b7c3`,
and helper SHA-256
`4286f3851d7022a11d7ae4cfefd21b264b0f3068342b99fac4f48e6e54d3784d`.
It passed preflight, loaded the models once in the helper, ran separate safe
unmodified CLI masked/unmasked comparisons, self-inspected, atomically
published, and exited zero. Its capture JSON reported manifest SHA-256
`8deedafa44c5f999e64d80dea21abbc9e4424e14cfa78339b345c8d4791e3f14`,
24 cases, and 226,621 bytes.

Required license review then added three omitted provenance URLs to the
deterministic corpus-local NOTICE template: the NCBI assembly report, upstream
gffutils database, and GENCODE checksum index. Only NOTICE and its manifest
size/digest declaration changed; `cases.jsonl` remained byte-identical and no
model was rerun. The final identities are the table above. The successful
capture is not rerun by normal validation.

A final execution-profile audit corrected only manifest/package metadata: the
legacy undifferentiated thread fields were replaced with
`helper_torch_intraop_threads=1`, `helper_torch_interop_threads=1`,
`cli_omp_threads=1`, and `cli_torch_interop_threads_observed=16`. The capture
preflight now performs bounded fresh-process probes for both roles before any
future inference. This did not change `cases.jsonl`, rerun the model, or alter
any numeric observation. The final manifest identity is the one in the table.

## Offline semantic and resource evidence

The final inspector returned exactly:

```json
{"status":"valid","schema":"pangopup-compat-v1","profile":"pangolin-1.0.2-5cf94b8-grch38-v1","cases":24,"scored_cases":14,"rejection_cases":6,"postprocess_cases":4,"coverage_cells":28}
```

Focused commands and observed results:

- `cargo test --locked -p pangopup-build compatibility`: eight unit controls
  passed, covering controlled masking/order/extrema, boundary-rule isolation,
  NumPy signed-zero tie behavior, f64 deletion no-narrowing, exact typed
  rendering, live helper/imported-module mutation, and all three atomic
  publication failure positions.
- `cargo test --locked -p pangopup-build --test compatibility`: seven
  integration tests passed. Hash-rebound raw-score, expected-position,
  non-extremal controlled-vector, masked-score, gene-order, context-anchor,
  exon-boundary, and rejection-category mutations reached the semantic
  inspector and failed. Line/context/gene bounds; duplicate/missing cases and
  checkpoints; DNA/strand/formula errors; malformed/wrong-width bits; missing
  license/coverage; provenance/schema/member shape; and typed/bounds mutations
  also failed closed.
- `/usr/bin/time -v target/debug/pangopup-build compatibility inspect ...`:
  0.02 seconds elapsed and 7,596 KiB maximum RSS on the development host.
- `/usr/bin/time -v cargo test --locked -p pangopup-build --test compatibility`:
  0.34 seconds elapsed, 40,096 KiB maximum RSS for Cargo plus the test process;
  the seven tests themselves reported 0.23 seconds.
- `mustmatch test spec/upstream-compatibility.md`: seven executable checks
  passed for valid summary, missing input, closed grammar, and a semantic
  expected-score mutation whose member digest was deliberately rebound.
- Full repository gates in the exact Pangopup workdir passed: `make lint`,
  `make test` (141 tests), and `make spec` (118 executable checks).

These timings are retained observations, not flaky pass/fail thresholds.

The new module changed Pangopup's embedded builder-source fingerprint, so the
checked 1,000-request SNV regression fixture was regenerated with its existing
deterministic generator. `scores.pgi` remained byte-identical at SHA-256
`fb0a77425456bd39e6aab7ad3447a24757f6889e82f7b27df01c214b78f8a6b9`,
and its fixed-v1 `NOTICE` remained byte-identical at SHA-256
`9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7`.
Only the manifest builder-source SHA, bundle identity
`ee6a50d5a1ef7e3eab2cd15a0334a6e117a86e367d9986271c1b5a09a945399b`,
and dependent expected provenance changed. The final builder-source SHA is
`c059bb409a49a3ddc0aefcf8a213b9685199a8ee4293366593cacd5a9f85829c`.
Compatibility attribution stays in the
corpus-local notice; the root notice remains the established fixed-v1 asset.

## Safety and limitations

The inspector opens the corpus directory no-follow, enumerates through that
opened directory, opens members relative to it with `openat` plus `NOFOLLOW`,
requires regular single-link files, checks member/aggregate bounds before
allocation, and enforces compact canonical closed JSON. Capture drains child
stdout/stderr without unbounded retention, hashes the exact live helper
immediately before spawn, and authenticates all tracked imported Pangolin
Python modules including the package initializer. P01-P03 are compared to
complete literal vectors/genes/expectations before independent replay. Capture
publishes through an absent sibling staging directory, syncs files/directories,
and renames no-replace. Capture, staging-sync, and rename failures clean only
that unpublished staging tree; a parent sync failure after successful rename is
reported while preserving the complete published output.

The corpus is an acceptance oracle, not a model runtime. It does not include
checkpoints, whole FASTA/GTF/SQLite inputs, inference code, tolerance policy,
or a complete all-gene masking-order representation. R05/R06 are deterministic
Rust slice-bound witnesses rather than CLI observations because the pinned
pyfastx native boundary path segfaulted during the superseded attempt. Exact
warning prose and process behavior remain documentary rather than Pangopup API.
The auxiliary CLI's observed inter-op default `16` is retained as provenance,
not generalized into a normative inference setting; only helper `1/1` produced
the raw arrays.

## Independent review and final gate

The independent code reviewer initially rejected five in-scope gaps: live
helper authentication, package-initializer authentication, literal P01-P03
pinning, late publication cleanup, and incomplete semantic/bounds controls.
The implementation author remediated all five without model execution or
corpus recapture. The same reviewer accepted implementation diff identity
`37d754b8056e80d5d21aeb4fe5906a52612b0a9b94c33b24c3d2b3580463e05e`
and found no issue to defer to another ticket.

The coordinator then rechecked member identities and the exact 24/14/6/4/28
inspector summary and reran the full repository gate in
`/home/ian/workspace/repos/pangopup`: `make lint` passed, `make test` passed all
141 tests, and `make spec` passed all 118 checks. The final gate did not invoke
Python, PyTorch, Pangolin, a network, external capture inputs, or production
SNV data.
