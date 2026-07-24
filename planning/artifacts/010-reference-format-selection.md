# Ticket 010 reference-format selection evidence

Status: complete; `acgt2-rle-v1` selected and independently accepted

The one retained run succeeded. Its complete output is preserved at:

```text
/home/ian/workspace/data/pangopup-reference-format-010/0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee/
```

Do not rerun an unchanged benchmark. The preserved `benchmark.json` is `2938`
bytes with SHA-256
`82badac6c02fa3830dc9014d3b8b378e19fe6265d269ca3ed88a52691cbc21af`.
The candidate manifest is `1016` bytes with SHA-256
`a2e77a972b9094cd069cbad20531d7a065175343e507ac3e468fcf17f87db478`.

## Result

The closed evaluator selected `acgt2-rle-v1` for `reason="speed"`. It was the
only candidate with at least a five-percent advantage over every opponent at
both headline quantiles:

| Codec | Headline p50 ns | Headline p95 ns | Mapped pages | Member bytes | Zstandard bytes | Allocations per copy |
|---|---:|---:|---:|---:|---:|---:|
| `ascii8` | 16,272 | 18,366 | 22 | 663,010,724 | 175,048,203 | 0 calls / 0 bytes |
| `iupac4` | 34,267 | 41,522 | 14 | 331,507,412 | 157,378,534 | 0 calls / 0 bytes |
| `acgt2-rle-v1` | 4,469 | 4,880 | 16 | 165,759,160 | 144,828,782 | 0 calls / 0 bytes |

The exact speed-rule comparisons for `acgt2-rle-v1` are:

```text
p50 vs ascii8:  4,469 * 100 = 446,900 <= 16,272 * 95 = 1,545,840
p95 vs ascii8:  4,880 * 100 = 488,000 <= 18,366 * 95 = 1,744,770
p50 vs iupac4: 4,469 * 100 = 446,900 <= 34,267 * 95 = 3,255,365
p95 vs iupac4: 4,880 * 100 = 488,000 <= 41,522 * 95 = 3,944,590
```

The winner therefore did not reach a tie-break. `iupac4` touched the fewest
logical pages, but page count is considered only when no material speed winner
exists. The selected two-bit payload was also the smallest installed member
and the smallest pinned Zstandard frame in this run.

## Raw retained summaries

All arrays are in fixed round order. Each round used 20 warmups and 10,000
retained M01–M14 cyclic operations. Times are integer nanoseconds.

```text
ascii8
  open: [27724, 32264, 40640, 30269, 46401]
  p50:  [16272, 16221, 16322, 16262, 16904]
  p95:  [18316, 18697, 18366, 18216, 18827]
  pages: [0,7079,7080,7081,7082,76318,76319,76320,82734,82735,
          82736,82737,82738,82739,82740,141379,141380,141381,
          143412,143413,143414,143415]

iupac4
  open: [31693, 36401, 31532, 31733, 35960]
  p50:  [34638, 35279, 34117, 34267, 33807]
  p95:  [41522, 41813, 39978, 41151, 41913]
  pages: [0,3540,3541,38159,38160,41367,41368,41369,41370,70690,
          70691,71706,71707,71708]

acgt2-rle-v1
  open: [37714, 39207, 37584, 32484, 37674]
  p50:  [5832, 4399, 4499, 4469, 4008]
  p95:  [6643, 4880, 4919, 4810, 4789]
  pages: [0,1770,1771,12104,19080,19081,20270,20684,20685,20686,
          28405,35346,35385,35854,35855,40467]
```

Every candidate copied the same `141521` logical bases per distinct-operation
trace and reported zero measured allocation calls and bytes.

## Identities

- reviewed contract:
  `0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee`;
- reviewed-ready base and `origin/main`:
  `e4895756f00bde46876f76fca231b328a484e668`;
- accepted pre-run 29-file inventory aggregate:
  `84a2daf1ffd039034b9ce6d55d19142dece40106cdb3ffe0b6af6aff2a1c9e6f`;
- source: `671294255` bytes, SHA-256
  `81645a227efbbd196ae337f743f31a5b1c32979d6d7bb5713e0322402a70fafb`;
- corpus manifest: `5337` bytes, SHA-256
  `fd12a0d6b503d1e572c0561eb43e66f19c55c4d073b25bced25be6303fd0553b`;
- corpus cases: `220071` bytes, SHA-256
  `2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8`;
- benchmark harness source SHA-256:
  `d6d3d706087a6725899831e0378a75526d699553aed542a3e6841f997f30a43e`;
- index candidate/evaluator source SHA-256:
  `58f79213da81f095b93df2f5047752e4904ebc9987507754afb5b0652e738b98`;
- builder candidate source SHA-256:
  `cfce86ff5ef3b2e236fb2cc8bbdeed0403b3edc2d2b81f9fb61ed72f460b5661`;
- release `pangopup-build` executable: `4665440` bytes, SHA-256
  `946b0a96b5803ad2ffa3f38f4d5c14a718bb384974c6925cc5842dca26a668c0`;
- release benchmark executable: `1639992` bytes, SHA-256
  `952b3b0aed86f88ee06fb1a05c0f92d8c0cd430b8871a1faf2fe3645e5613d57`;
- locked dependency graph SHA-256:
  `6cedbe1baf5fd6b43eb8f394f582dde1f265750e926517004c216c55f0e0fc3d`.

Candidate member identities are:

| Codec | SHA-256 |
|---|---|
| `ascii8` | `43b329682bed49408cf0efaffd9e295ce1ebf04f3eb879f2494b98a954086a25` |
| `iupac4` | `c2355c54e6d0d7767a13286c0fd364b8bc22ecada14c01d3977824ee3b807258` |
| `acgt2-rle-v1` | `fb98ca0e5fb76307897e9fb3d6950629d211d3f464ec32f2ce535aac54c793c0` |

The three members plus manifest occupy `1,160,278,312` bytes. The candidate
container is `pgrben01-v1`, uses 4096-byte logical pages, and contains contig
codes `[3,10,12,13,17,25]`.

After the run succeeded, finalization changed Markdown decision, evidence, and
current/future documentation only. The causal benchmark, builder, reader,
evaluator, fixture, dependency lock, and release executables retain the exact
identities above. No source or lockfile change invalidated the accepted run.

## Host and resources

The retained host reported Rust `1.93.1`, target
`x86_64-unknown-linux-gnu`, Linux kernel `6.17.0-35-generic`, and an AMD Ryzen
7 5825U. The benchmark was pinned to one logical CPU with affinity `0`; the
host reported the `powersave` policy. No major faults or swaps occurred.
Immediately before launch, `df -B1` reported `180393086976` available bytes on
the output filesystem. Power-supply status was not exposed under
`/sys/class/power_supply`; no AC/battery claim is made.

| Phase | Wall | User | System | Maximum RSS | Minor faults | Major faults |
|---|---:|---:|---:|---:|---:|---:|
| prepare | 26.04 s | 24.43 s | 1.41 s | 651,684 KiB | 15,585 | 0 |
| inspect | 1.59 s | 1.35 s | 0.24 s | 651,628 KiB | 15,555 | 0 |
| benchmark (`time`) | 62.35 s | 61.52 s | 0.67 s | 650,280 KiB | 29,161 | 0 |
| benchmark report | — | — | — | 665,886,720 bytes | 23,350 | 0 |

The high RSS is reported, not hidden. These phases exhaustively inspect large
memory-mapped members, so Linux counts resident file-backed mmap pages in RSS.
It is not evidence that the builder or per-request decoder retained roughly
650 MiB of heap. The measured copy path itself allocated zero bytes. Peak
candidate-preparation scratch-file length was deterministically `1280` bytes:
the preserved two-bit directory records per-contig ambiguity-run counts
`[29,80,30,23,48,1]`, the reviewed writer truncates its sibling scratch at each
contig, and every run record is exactly 16 bytes (`80 * 16 = 1280`). This is the
peak run-scratch file only; it is distinct from private staging and the
`1,160,278,312` published candidate-set bytes.

## Exact retained commands

Each command below was run once under `/usr/bin/time -v`; its complete output
is preserved beside the report.

```text
target/release/pangopup-build reference-candidates prepare \
  --source /home/ian/workspace/data/pangopup-compat-inputs/refseq-grch38p14-compat-six-contigs.fa \
  --corpus tests/fixtures/pangolin-compat-v1 \
  --output /home/ian/workspace/data/pangopup-reference-format-010/0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee/candidates

target/release/pangopup-build reference-candidates inspect \
  --candidates /home/ian/workspace/data/pangopup-reference-format-010/0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee/candidates \
  --corpus tests/fixtures/pangolin-compat-v1

env \
  PANGOPUP_REFERENCE_CANDIDATES=/home/ian/workspace/data/pangopup-reference-format-010/0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee/candidates \
  PANGOPUP_REFERENCE_CORPUS=/home/ian/workspace/repos/pangopup/tests/fixtures/pangolin-compat-v1 \
  PANGOPUP_REFERENCE_REPORT=/home/ian/workspace/data/pangopup-reference-format-010/0b59f1b874c6e2bc7c3a7943febd1d1041e613b7e0af4f413f21602c92ebbfee/benchmark.json \
  target/release/deps/reference_formats-562045622d793684
```

`prepare.stdout.jsonl` and `inspect.stdout.jsonl` retain the successful typed
CLI observations. `prepare.log`, `inspect.log`, and `benchmark.log` retain the
resource records.

Before preparation, the coordinator explicitly checked that `candidates/`,
`benchmark.json`, `prepare.log`, and `prepare.stdout.jsonl` did not exist. The
contract-hash root was then created only to retain this job's logs; candidate
publication still targeted an absent child directory and report publication an
absent file. Unified execution session `90779` and supervising interactive zsh
PID `3895136` were recorded before work began. The completed phase PIDs were
`3907614` (prepare), `3911955` (inspect), and `3915772` (benchmark); none remains
active.

Progress was observed without restart through session `90779`, those PID
files, process state, staging/member sizes, and final stdout/log/report files.
The recorded cancellation procedure was to send Ctrl-C to session `90779`,
wait for its foreground phase to exit, and inspect retained logs/output before
any decision; an unchanged failed phase was not to be retried automatically.
The session exited normally after success. Standard benchmark output is empty
by contract. Unpublished staging cleanup belonged to the program; published
candidates/report and all logs were to be preserved for diagnosis. The
successful unchanged candidate/report are preserved and must never be rerun.

## Interpretation and limitations

- This selects the payload encoding for production hardening. Ticket 010 does
  not ship a production reference bundle, reader/provider, installer, or
  release asset.
- The workload contains the exact 14 compatibility contexts on six pinned
  RefSeq GRCh38.p14 contigs. It is representative of the accepted model oracle,
  not a full 25-primary-sequence production build or a broad workload survey.
- The result is one-host, one-CPU, warm/page-cache evidence. It makes no cold
  I/O, multi-core scaling, MPS, CUDA, or model-inference claim.
- The page trace is deterministic logical decoder work, not an observation of
  physical disk reads. RSS includes file-backed page residency as described
  above.
- Zstandard sizes use the pinned whole-member settings only as the final
  download-size tie-break. They do not define a production transport.
- The selected benchmark container remains isolated. The next ticket must
  harden `acgt2-rle-v1` for the complete 25-primary-sequence runtime asset and
  independently verify its production manifest, builder, reader, bounds,
  corruption behavior, memory/page behavior, and delivery contract.

The miniature fixture remains the normal test oracle with candidate-set
SHA-256 `557cfa37dda0cb7d89b552d2e3cb2a3c31ebea26a937f386ff149e6ed17c08ff`.
No normal gate reads this retained directory.
