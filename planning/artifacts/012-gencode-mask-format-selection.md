# Ticket 012 GENCODE mask-format selection evidence

Status: complete; `domains` selected mechanically for production hardening

The one retained full-source comparison succeeded. Its complete private stage
is preserved read-only at:

```text
/home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb/
```

Do not rerun an unchanged capture, preparation, or benchmark. Git retains only
the bounded [benchmark report](012-benchmark-report.json) and exact
[1,000-query manifest](012-performance-manifest.jsonl). Their identities are:

| Evidence | Bytes | SHA-256 |
|---|---:|---|
| benchmark report | 7,951 | `e064666b604f027d8b7c88eaa34c4751bbdbac0a14fa18086cf2d88007c82722` |
| performance manifest | 163,320 | `ffcf61425a69546c79405c8bbfe01cda77c86051d6828a05a74fe4b45e6c1473` |

The raw database, GTF, observation, canonical stream, candidate members, and
receipts remain private build evidence and are not repository or runtime
assets.

## Result

The closed speed-first selector chose the constant-membership `domains`
candidate. All three candidates first passed exact semantic certification,
corruption controls, zero-allocation warmed lookup, and deterministic logical
page tracing.

| Codec | Headline p50 ns | Headline p95 ns | Payload pages median/p95 | Member bytes | Pinned Zstandard bytes | Open peak Rust heap |
|---|---:|---:|---:|---:|---:|---:|
| `interval-tree` | 241 | 401 | 8 / 11 | 5,763,120 | 3,554,641 | 1,112 |
| `domains` | 171 | 331 | 7 / 9 | 6,703,320 | 3,933,486 | 1,112 |
| `binned-postings` | 241 | 431 | 6 / 7 | 5,759,360 | 3,393,086 | 1,112 |

The first selector step found a minimum p95 of 331 ns and retained only
`domains` under the five-percent window:

```text
domains:         331 * 100 = 33,100 <= 331 * 105 = 34,755
interval-tree:   401 * 100 = 40,100 >  331 * 105 = 34,755
binned-postings: 431 * 100 = 43,100 >  331 * 105 = 34,755
```

Later p50, page, heap, member-size, compressed-size, and fixed-simplicity
steps therefore could not change the survivor. The choice is intentionally
speed-first: `domains` was the largest installed and compressed candidate,
while `binned-postings` touched the fewest payload pages.

## Raw retained rounds

Each candidate ran once at every schedule position across six balanced rounds.
Each round used 10,000 warmups followed by 100,000 timed queries cycling the
same 1,000-query manifest. Times are integer nanoseconds.

| Codec | Round | Schedule position | Open ns | p50 ns | p95 ns |
|---|---:|---:|---:|---:|---:|
| `interval-tree` | 0 | 0 | 13,046 | 240 | 400 |
| `interval-tree` | 1 | 0 | 13,586 | 241 | 401 |
| `interval-tree` | 2 | 1 | 8,517 | 241 | 401 |
| `interval-tree` | 3 | 2 | 8,897 | 240 | 391 |
| `interval-tree` | 4 | 1 | 8,617 | 241 | 410 |
| `interval-tree` | 5 | 2 | 7,564 | 241 | 420 |
| `domains` | 0 | 1 | 13,737 | 180 | 340 |
| `domains` | 1 | 2 | 9,388 | 180 | 341 |
| `domains` | 2 | 0 | 16,572 | 180 | 331 |
| `domains` | 3 | 0 | 13,156 | 171 | 330 |
| `domains` | 4 | 2 | 9,178 | 160 | 311 |
| `domains` | 5 | 1 | 14,999 | 160 | 331 |
| `binned-postings` | 0 | 2 | 13,337 | 241 | 431 |
| `binned-postings` | 1 | 1 | 8,667 | 241 | 441 |
| `binned-postings` | 2 | 2 | 8,126 | 241 | 431 |
| `binned-postings` | 3 | 1 | 9,388 | 240 | 411 |
| `binned-postings` | 4 | 0 | 11,773 | 240 | 411 |
| `binned-postings` | 5 | 0 | 12,134 | 241 | 431 |

Every warmed round reported zero allocation calls and bytes, zero measured
minor or major faults inside the candidate block, and the same 1,112-byte open
heap peak. Headline values are nearest-rank p50s of the six per-round
quantiles, using sorted index 2 as fixed by the ticket.

Candidate member identities are:

| Codec | SHA-256 | Logical page-trace SHA-256 |
|---|---|---|
| `interval-tree` | `c931550016aebfb5da341a066d00fe0f8819e75a9c313fa646356125bda2bd7f` | `7fb1980c483c93522019d56ff5a6bc701a634439a3e0811265d893bdd6b68b71` |
| `domains` | `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702` | `fbd8a8b4eefee2cf8364d65e6b080b9a44ea86e33330755ba42fa1a6d57bb25d` |
| `binned-postings` | `06d203148e97bc8c1f2b58fd59e9a41ed83afbaa4088479746b37fe9e1fb25b4` | `2309d29007ba68faea2234f5a63e940a489704615541245ddde40ed78886f165` |

Pinned compression was
`zstd-0.13.3/libzstd-1.5.7;level=9;checksum;content-size;no-dict-id;no-long-distance;workers=0`.

## Logical source and workload

The exact profile is `pangolin-1.0.2-5cf94b8-grch38-v1`; the mask-local
builder identity is
`fd738fecac360867b74ec786dc53366e05ed1f78ef76062476a136feefe76816`.
The final capture contract is 80,783 bytes with SHA-256
`ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb`.

Authenticated source and semantic identities are:

| Input/evidence | Bytes | SHA-256 |
|---|---:|---|
| GENCODE v38 gffutils database | 380,366,848 | `221a61eec1f6934ae426d80599989c7b2ee4d9577b52e8a0e4bf02ccd73ca4a6` |
| GENCODE v38 GTF gzip | 46,556,621 | `22020df0d3356e965868f4b193e89fa13e838b950a574349f7fcd461ac01c050` |
| observation helper | 12,826 | `950ec5d89b57c4e5ad39f621053182acdcbb0c4cf1407abe8455504688d4a8d3` |
| Python 3.13.5 executable | 34,679,464 | `c243a3ad6dc86fcde244245aca621adee9766759c7524ca89f1b3a44ff4fdc24` |
| generic venv launcher symlink payload | 79 | `b407404b75f49e4f39686a8a060bdcbadae49e5c5f1ecd5d6e593f9468bc8ffe` |
| `pyvenv.cfg` | 171 | `b39b62bae0935628201c24541bebd3011ff5527b55070543a4c565542d8b2ba9` |
| captured Python/SQLite environment | 79,641 | `3f2a7837571b0cfc0827ad11a9f313b6ecfdbc358e3b9868e3ede748ccaa2fed` |
| upstream observation | 22,947,218 | `b1dcda39ecb9960a282ba74bb7147dd3536d724ece87fa2cb581afa4ed7b6cac` |
| canonical logical stream | 29,322,409 | `a23a24b9b421cc6790111cfb852ea375169d98cbeec0bcf4469194f412ee3014` |
| compatibility corpus | 227,060 | `c077d400230fc7df83242d2737a850b2709299be990f521599b0e55735ff55e3` |
| compatibility points | 1,200 | `2356eeaad935f3cbb572ab4fe333f7009b4f053c6069b89825725645bf813d32` |

The captured environment reports gffutils 0.14, Python sqlite3 module 2.6.0,
SQLite 3.49.1, and 254 authenticated modules. Its exact query plan is
`SEARCH features USING INDEX seqidstartend (seqid=?)`.

The canonical inventory contains 60,649 exact versioned genes on 25 primary
contigs, 88,202 constant-membership domains, and 591,404 normalized exon
boundaries. It contains 30,769 plus-strand and 29,880 minus-strand genes;
60,605 distinct stable IDs; 44 `_PAR_Y` genes and 44 corresponding stable-ID
collisions; no duplicate exact ID and no boundary-empty gene. The maximum is
726 boundaries for one gene. There are 14,314 same-strand and 20,368
opposite-strand multi-gene domains.

The performance manifest is JSON Lines: one header followed by 1,000 ordered
queries with expected-result SHA-256 values. Its fixed strata are:

| Stratum | Queries | Distinct | Repeated | Eligible |
|---|---:|---:|---:|---:|
| single gene | 486 | 486 | 0 | 56,742 |
| no gene | 100 | 100 | 0 | 32,782 |
| same-strand multi-gene | 100 | 100 | 0 | 14,314 |
| opposite-strand multi-gene | 100 | 100 | 0 | 20,368 |
| boundary start | 25 | 25 | 0 | 60,504 |
| boundary start + 1 | 25 | 25 | 0 | 60,504 |
| boundary end | 25 | 25 | 0 | 60,466 |
| boundary end + 1 | 25 | 25 | 0 | 60,466 |
| pseudoautosomal pair | 88 | 88 | 0 | 88 |
| compatibility | 14 | 8 | 6 | 14 |
| extreme cardinality | 12 | 12 | 0 | 88,202 |

Candidate correctness was not sampled from this performance set. Preparation
also compared every candidate against the independent logical source at one
witness in every domain plus every `start`, `start+1`, `end`, and `end+1`
edge.

## Host and resources

The retained release run used Rust
`rustc 1.93.1 (01f6ddf75 2026-02-11)`, target
`x86_64-unknown-linux-gnu`, Linux `6.17.0-35-generic`, and an AMD Ryzen 7
5825U with Radeon Graphics. The process inherited 16 allowed CPUs and pinned
CPU 0. The host reported governor `powersave`, power state
`balance_performance`, and a 4,096-byte logical page.

The release executable is 3,948,640 bytes with SHA-256
`d1770de9892ff0ead5d2d716f4cfc4ea1919077312493de46be5d784cdfa050c`.
The complete benchmark process reported 351,670,272 bytes maximum RSS, 15,338
minor faults, and no major fault. This RSS includes pages touched by exhaustive
certification and all three candidates; it is not a per-query heap claim.

## Exact retained commands

The first capture sealed the source observation under builder
`5f248285fdccb613142a504ea172d7de4f61ea0cf92acd9ffcb0c0bc29c37970`
with this exact invocation:

```text
target/release/pangopup-mask-candidates capture \
  --database /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.db \
  --gtf /home/ian/workspace/data/pangopup-compat-inputs/gencode.v38.annotation.gtf.gz \
  --python /home/ian/.local/share/uv/python/cpython-3.13.5-linux-x86_64-gnu/bin/python3.13 \
  --python-bytes 34679464 \
  --python-sha256 c243a3ad6dc86fcde244245aca621adee9766759c7524ca89f1b3a44ff4fdc24 \
  --python-launcher /home/ian/.local/share/uv/tools/pangolin/bin/python \
  --python-launcher-link-bytes 79 \
  --python-launcher-link-sha256 b407404b75f49e4f39686a8a060bdcbadae49e5c5f1ecd5d6e593f9468bc8ffe \
  --pyvenv-config-bytes 171 \
  --pyvenv-config-sha256 b39b62bae0935628201c24541bebd3011ff5527b55070543a4c565542d8b2ba9 \
  --output-parent /home/ian/workspace/data/pangopup-mask-qualification-012
```

After a prepare-only parser defect was fixed, the read-only planner derived the
one-field contract change:

```text
target/release/pangopup-mask-candidates plan-capture-promotion \
  --prior-stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab \
  --source-builder-sha256 5f248285fdccb613142a504ea172d7de4f61ea0cf92acd9ffcb0c0bc29c37970
```

Independent authorization then allowed only the following cross-builder
capture promotion:

```text
target/release/pangopup-mask-candidates promote-capture \
  --prior-stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-2f6f5bf034e713b49a04a527f75b4061ac2fc25b680694e1509553ff95f7fcab \
  --output-parent /home/ian/workspace/data/pangopup-mask-qualification-012 \
  --authorization /home/ian/workspace/data/pangopup-mask-qualification-012/capture-promotion-2f6f5bf-to-ce035636.json
```

The promoted stage was then prepared, inspected read-only, benchmarked once,
published no-replace, and inspected read-only at the final path:

```text
target/release/pangopup-mask-candidates prepare \
  --stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb-promotion-2f864eb2e73682700b34c729ffdecc26c6f3a7ffb3a3d3d719db2a793d935fa9 \
  --compatibility-corpus /home/ian/workspace/repos/pangopup/tests/fixtures/pangolin-compat-v1

target/release/pangopup-mask-candidates inspect \
  --stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb-promotion-2f864eb2e73682700b34c729ffdecc26c6f3a7ffb3a3d3d719db2a793d935fa9

target/release/pangopup-mask-candidates benchmark \
  --stage /home/ian/workspace/data/pangopup-mask-qualification-012/.pangopup-mask-stage-ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb-promotion-2f864eb2e73682700b34c729ffdecc26c6f3a7ffb3a3d3d719db2a793d935fa9

target/release/pangopup-mask-candidates inspect \
  --stage /home/ian/workspace/data/pangopup-mask-qualification-012/ce0356365d65ef2d0a0d5415d917e2b3ca03ca47b1bf05a75b3e736a2c8907fb
```

The promotion authorization is 827 bytes with SHA-256
`2f864eb2e73682700b34c729ffdecc26c6f3a7ffb3a3d3d719db2a793d935fa9`.
Final capture, prepare, and benchmark receipts are respectively
1,467/1,567/1,013 bytes with SHA-256
`263dd8ea9466e3019c0b75e907d9c4ed62aaf8fcbf849759ac144994c9c9c057`,
`8fd3b24b060706458d46e5778d4a55b82ffa9e3f92eb8276d424c5925c1a2f0e`,
and `19e4a912f1cd7a46f0a401f316270f35229f105ba6d17881c4fb32cb6c0aea10`.

## Interpretation and limitations

- This selects a logical encoding for production hardening. `PGMBEN01` remains
  a private benchmark-only family and is not a supported runtime format.
- There is still no production mask magic, manifest, builder, bundle, typed
  provider, installer, transport, remote asset, or model integration.
- The retained timing is one-host, one-CPU, warm/page-cache evidence. It makes
  no cold-I/O, multi-core, accelerator, HTTP, or model-inference claim.
- Logical page traces are deterministic decoder work, not physical disk reads.
- Pinned Zstandard sizes are a final selection tie-break only; they do not
  specify a production mask transport.
- The closeout audit independently hashed the bounded contract, receipts,
  inventory, workload, and report. It reconciled the large source, observation,
  canonical, and candidate identities through their authenticated contract and
  receipt chain rather than needlessly rereading those payloads.
- The next outcome must re-specify `domains` in a distinct production format,
  build all 25 contigs reproducibly, preserve exact order/identity semantics,
  and qualify cheap open, point lookup, corruption handling, bounded memory,
  license notices, and bundle identity before any runtime or release claim.

Normal gates use the independent miniature and never read the private stage or
rerun production qualification. The bounded JSON evidence is retained for
review and reproducibility, not as a routine verifier input.
