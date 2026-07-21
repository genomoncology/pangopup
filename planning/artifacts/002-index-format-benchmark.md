# Ticket 002 index-format benchmark

Date: 2026-07-21
Status: retained comparative warm evidence; fixed 11-byte v1 selected

## Outcome

The fixed 11-byte locus representation won the accepted priority ordering. It
was the fastest candidate for one-open 1, 10, and 100-record workloads and for
reopen-plus-query after the hierarchical candidate was corrected to use
zero-copy mmap reads, packed six-bit masks, and 64-locus rank checkpoints. On
the equal candidate harness, fixed measured 121 / 972 / 9,949 ns versus direct
at 160 / 1,243 / 14,749 ns for one-open 1/10/100. The separately hardened
product reader measured 210 / 1,964 / 19,588 ns. Tabix did not trigger the stop
branch: its corresponding p50 was 9.372 / 97.298 / 978.623 ms.

This result intentionally spends installed bytes for query speed. On the lab
corpus, the promoted fixed artifact was 108,467,071 bytes, versus 13,844,635
bytes for hierarchical direct. Page work therefore favors direct, but query
latency is the first accepted priority and the fixed speed win is clear.

## Reproduction and inputs

The benchmark target is owned by `pangopup-index`. From the repository root:

```bash skip
PANGOPUP_SOURCE_DIR=/path/to/Pangolin_hg38_snvs_masked
cargo run --release --locked -p pangopup-build -- \
  benchmark-corpus "$PANGOPUP_SOURCE_DIR" \
  target/index-benchmark/corpus.pglog \
  target/index-benchmark/selected-genes.tsv
PANGOPUP_BENCH_CORPUS="$PWD/target/index-benchmark/corpus.pglog" \
PANGOPUP_BENCH_OUTPUT="$PWD/target/index-benchmark/run" \
PANGOPUP_BENCH_ITERATIONS=20 \
cargo bench --locked -p pangopup-index --bench index_formats
```

The source directory is operator-supplied and is not treated as proof of the
published ZIP checksum. Published metadata is recorded separately:

| Field | Value |
|---|---|
| DOI | `10.5281/zenodo.15649338` |
| Archive | `Pangolin_hg38_snvs_masked.zip` |
| Published archive bytes | 12,988,141,317 |
| Publisher MD5 | `679ef0b50e511b6102b4b88fbf811108` |
| Selected extracted members | 134 |
| Selected loci / rows | 9,858,991 / 29,576,973 |
| Observed framed member-set SHA-256 | `0852168353c8d309a1850bc64049eb8591b4097c5e8635d1042458c5c37c261b` |

The observed digest is over sorted UTF-8 member names using repeated
`u64_le(name_len) || name || u64_le(member_len) || member_bytes` frames. The
exact 6 required edge genes plus 16 evenly spaced filenames from each of the 8
direction × compressed-size-quartile strata are in
[`002-selected-genes.tsv`](002-selected-genes.tsv). The exact primary requests
are in [`002-query-manifest.tsv`](002-query-manifest.tsv).

No optional archive path was supplied, so no local archive hash is reported.

## Machine and method

- base commit: `68e349bb32cb9d2986c44d73ee547a3d110e2030` plus the Ticket 002 working diff;
- compiler: `rustc 1.93.1 (01f6ddf75 2026-02-11)`, LLVM 21.1.8, release profile;
- CPU: AMD Ryzen 7 5825U, 8 cores / 16 threads, 16 MiB L3;
- memory: 27.3 GiB;
- OS: Ubuntu 24.04, Linux 6.17.0-35-generic x86_64;
- storage: Crucial CT1000P3PSSD8 1 TB NVMe, ext4;
- samples: 20 retained samples after 20 warmups per codec and mode;
- cache label: warm operating-system page cache;
- Tabix: `/usr/bin/bgzip` + `/usr/bin/tabix`, with an in-process noodles
  indexed reader; a reader/index stays alive for `one-open`, a fresh in-process
  handle is created per `reopen-plus-query` sample, and returned rows are parsed;
- timing excludes CLI, subprocess, and stdout from query samples;
- allocation calls use a counting global allocator;
- page faults use `getrusage(RUSAGE_SELF)` deltas;
- percentiles use the nearest-rank convention: sorted sample at
  `ceil(p * sample_count)`, one-based. With 20 samples p99 is therefore the
  maximum rather than the same nonmaximum order statistic as p95;
- throughput is complete workload samples per second, not records per second;
- logical bytes are decoder-instrumented encoded bytes. Page counts use one
  convention for every mmap codec: unique 4 KiB mapped artifact page numbers
  addressed across one complete workload sample. Reopen rows report query-phase
  bytes/pages so they remain comparable to one-open; open work has its own row.
  Tabix has no mmap artifact metric and is shown as unavailable. Neither metric
  claims physical storage bytes read;
- the direct kernel reads its mapped payload without copying, uses 4,096-locus
  blocks, packed six-bit masks, and active/pair rank checkpoints every 64 loci.

The selected-reader figures below are the hardened product implementation. The
separate `fixed-11` benchmark kernel is retained as an independent check.

## Artifact construction and open

| Codec | Artifact bytes | Serialization ms |
|---|---:|---:|
| selected fixed-11 | 108,467,071 | 1,492.328 |
| hierarchical direct | 13,844,635 | 721.492 |
| fixed-11 kernel | 108,463,495 | 946.993 |
| Zstd 1,024 | 11,330,887 | 844.935 |
| Zstd 2,048 | 10,800,344 | 773.555 |
| Zstd 4,096 | 10,473,652 | 759.213 |
| LZ4 1,024 | 13,118,036 | 743.193 |
| LZ4 2,048 | 12,769,422 | 756.710 |
| LZ4 4,096 | 12,484,585 | 738.566 |
| Tabix | 89,884,205 | 22,569.661 |

Open timings are p50 / p95 / p99 ns. `A`, `L`, `P`, and `F` mean allocations,
logical bytes, unique pages, and minor/major faults per sample.

| Codec | Open ns | Samples/s | A | L | P | F |
|---|---:|---:|---:|---:|---:|---:|
| selected fixed-11 | 20,490 / 20,991 / 21,753 | 48,836.6 | 2 | 50,064 | 6 | 2 / 0 |
| hierarchical direct | 23,556 / 24,067 / 30,530 | 41,728.7 | 4 | 89,352 | 22 | 1 / 0 |
| fixed-11 kernel | 15,230 / 21,913 / 22,434 | 59,302.1 | 4 | 14,536 | 4 | 1 / 0 |
| Zstd 1,024 | 50,589 / 51,041 / 56,141 | 19,631.5 | 4 | 320,520 | 79 | 1 / 0 |
| Zstd 2,048 | 32,244 / 32,524 / 32,664 | 31,008.6 | 4 | 166,472 | 41 | 1 / 0 |
| Zstd 4,096 | 22,214 / 23,236 / 27,885 | 44,182.0 | 4 | 89,352 | 22 | 1 / 0 |
| LZ4 1,024 | 53,596 / 54,368 / 65,499 | 18,478.4 | 4 | 320,520 | 79 | 1 / 0 |
| LZ4 2,048 | 31,713 / 32,664 / 37,865 | 31,150.9 | 4 | 166,472 | 41 | 1 / 0 |
| LZ4 4,096 | 21,773 / 21,863 / 21,984 | 45,959.3 | 4 | 89,352 | 22 | 1 / 0 |
| Tabix | 2,447,297 / 2,516,985 / 2,683,484 | 409.7 | 1,141 | — | — | 0 / 0 |

## Warm one-open results

Each request is distinct and gene-filtered, so the workloads return exactly
1/10/100 records. Timings are p50 / p95 / p99 ns.

| Codec | 1 request | 10 requests | 100 requests |
|---|---:|---:|---:|
| selected fixed-11 | 210 / 231 / 280 | 1,964 / 2,074 / 2,084 | 19,588 / 19,890 / 19,940 |
| hierarchical direct | 160 / 181 / 190 | 1,243 / 1,253 / 1,273 | 14,749 / 15,520 / 15,801 |
| fixed-11 kernel | 121 / 230 / 230 | 972 / 1,002 / 1,012 | 9,949 / 13,537 / 14,378 |
| Zstd 1,024 | 4,318 / 4,378 / 4,408 | 46,161 / 46,582 / 49,839 | 462,132 / 481,330 / 695,231 |
| Zstd 2,048 | 4,338 / 4,378 / 4,419 | 46,782 / 46,972 / 102,662 | 465,638 / 474,857 / 477,933 |
| Zstd 4,096 | 4,359 / 4,408 / 4,439 | 42,594 / 42,874 / 47,664 | 469,827 / 501,910 / 533,182 |
| LZ4 1,024 | 2,535 / 2,566 / 2,605 | 26,773 / 26,873 / 26,883 | 274,101 / 280,213 / 287,949 |
| LZ4 2,048 | 2,525 / 2,555 / 2,595 | 26,492 / 26,623 / 31,883 | 277,348 / 281,866 / 299,101 |
| LZ4 4,096 | 2,454 / 2,505 / 2,525 | 24,508 / 24,558 / 24,589 | 249,732 / 273,640 / 277,588 |
| Tabix | 9,372,057 / 9,544,597 / 10,325,337 | 97,298,171 / 101,255,523 / 106,140,518 | 978,623,393 / 1,077,987,993 / 1,145,890,372 |

The following cells are `samples/s; A; L; P; F` for the same workloads.

| Codec | 1 request | 10 requests | 100 requests |
|---|---:|---:|---:|
| selected fixed-11 | 4,653,327; 2; 915; 4; 0/0 | 503,550; 20; 9,150; 5; 0/0 | 50,976.8; 200; 91,500; 5; 0/0 |
| hierarchical direct | 6,144,393; 3; 30; 1; 0/0 | 804,505; 28; 228; 1; 0/0 | 67,430; 255; 1,998; 1; 0/0 |
| fixed-11 kernel | 6,700,167; 3; 11; 1; 0/0 | 1,023,018; 30; 110; 1; 0/0 | 92,675.8; 300; 1,100; 2; 0/0 |
| Zstd 1,024 | 231,645; 4; 588; 2; 0/0 | 21,571.8; 40; 5,880; 2; 0/0 | 2,107.6; 400; 58,800; 2; 0/0 |
| Zstd 2,048 | 230,383; 4; 588; 1; 0/0 | 20,204.6; 40; 5,880; 1; 0/0 | 2,143.4; 400; 58,800; 1; 0/0 |
| Zstd 4,096 | 229,019; 4; 588; 1; 0/0 | 23,325.8; 40; 5,880; 1; 0/0 | 2,104.6; 400; 58,800; 1; 0/0 |
| LZ4 1,024 | 393,229; 4; 639; 1; 0/0 | 37,352.6; 40; 6,390; 1; 0/0 | 3,627.4; 400; 63,900; 1; 0/0 |
| LZ4 2,048 | 395,491; 4; 639; 1; 0/0 | 37,455; 40; 6,390; 1; 0/0 | 3,588.7; 400; 63,900; 1; 0/0 |
| LZ4 4,096 | 406,769; 4; 639; 1; 0/0 | 40,795.8; 40; 6,390; 1; 0/0 | 3,940.3; 400; 63,900; 1; 0/0 |
| Tabix | 106.1; 113,591; —; —; 0/0 | 10.3; 1,135,910; —; —; 0/0 | 1.0; 11,359,100; —; —; 0/0 |

## Warm reopen-plus-query results

Timings include a fresh in-process open in every sample; `L/P` below remains
the query phase so it uses the same metric as one-open.

| Codec | 1 request | 10 requests | 100 requests |
|---|---:|---:|---:|
| selected fixed-11 | 20,570 / 21,272 / 27,504 | 22,094 / 22,705 / 22,734 | 39,588 / 56,712 / 61,451 |
| hierarchical direct | 24,208 / 25,160 / 29,929 | 24,970 / 25,641 / 30,991 | 35,660 / 35,981 / 36,382 |
| fixed-11 kernel | 16,533 / 24,469 / 27,444 | 16,583 / 24,629 / 27,725 | 27,805 / 36,121 / 40,350 |
| Zstd 1,024 | 56,231 / 56,862 / 61,452 | 99,607 / 103,283 / 103,455 | 522,461 / 561,267 / 678,178 |
| Zstd 2,048 | 37,654 / 38,386 / 42,093 | 80,970 / 82,543 / 84,046 | 516,339 / 534,745 / 546,017 |
| Zstd 4,096 | 27,114 / 27,885 / 32,965 | 64,998 / 66,010 / 69,007 | 476,000 / 483,043 / 517,181 |
| LZ4 1,024 | 57,072 / 67,884 / 68,355 | 81,521 / 86,090 / 86,702 | 328,298 / 337,796 / 375,050 |
| LZ4 2,048 | 35,430 / 35,771 / 36,422 | 59,878 / 60,920 / 65,629 | 315,503 / 320,562 / 356,193 |
| LZ4 4,096 | 24,679 / 25,059 / 25,219 | 46,602 / 52,814 / 54,217 | 291,585 / 335,131 / 347,596 |
| Tabix | 11,954,740 / 12,392,855 / 12,500,166 | 99,461,953 / 102,458,694 / 103,297,570 | 970,882,541 / 1,022,082,409 / 1,235,560,995 |

The following cells are `samples/s; A; L; P; F`.

| Codec | 1 request | 10 requests | 100 requests |
|---|---:|---:|---:|
| selected fixed-11 | 47,623.7; 4; 915; 4; 2/0 | 45,152.7; 22; 9,150; 5; 2/0 | 23,473.7; 202; 91,500; 5; 2/0 |
| hierarchical direct | 40,700.1; 7; 30; 1; 1/0 | 39,525.8; 32; 228; 1; 1/0 | 28,029.8; 259; 1,998; 1; 1/0 |
| fixed-11 kernel | 54,243.3; 7; 11; 1; 1/0 | 54,662.3; 34; 110; 1; 1/0 | 35,091; 304; 1,100; 2; 1/0 |
| Zstd 1,024 | 17,695.8; 8; 588; 2; 1/0 | 10,006.3; 44; 5,880; 2; 1/0 | 1,879; 404; 58,800; 2; 1/0 |
| Zstd 2,048 | 26,400; 8; 588; 1; 1/0 | 12,395.9; 44; 5,880; 1; 1/0 | 1,928.9; 404; 58,800; 1; 1/0 |
| Zstd 4,096 | 36,387.1; 8; 588; 1; 1/0 | 15,314.7; 44; 5,880; 1; 1/0 | 2,089.5; 404; 58,800; 1; 1/0 |
| LZ4 1,024 | 16,993.5; 8; 639; 1; 1/0 | 12,186.2; 44; 6,390; 1; 1/0 | 3,005.5; 404; 63,900; 1; 1/0 |
| LZ4 2,048 | 28,165; 8; 639; 1; 1/0 | 16,594.2; 44; 6,390; 1; 1/0 | 3,146; 404; 63,900; 1; 1/0 |
| LZ4 4,096 | 40,454.5; 8; 639; 1; 1/0 | 21,118.5; 44; 6,390; 1; 1/0 | 3,309.9; 404; 63,900; 1; 1/0 |
| Tabix | 83.3; 114,735; —; —; 0/0 | 10.0; 1,137,054; —; —; 0/0 | 1.0; 11,360,244; —; —; 0/0 |

## Special workloads

Times are p50 / p95 / p99 ns. Same/cross block contain ten hits;
gene-filtered and `REF=N` return one outcome, all-overlap returns two, and
absent returns none.

| Codec | Same block | Cross block | Gene-filtered | All overlap | Absent | `REF=N` |
|---|---:|---:|---:|---:|---:|---:|
| selected fixed-11 | 1,924/1,984/2,034 | 2,515/2,545/2,615 | 220/240/251 | 200/211/221 | 180/201/210 | 300/321/321 |
| hierarchical direct | 1,554/1,603/1,623 | 1,132/1,162/1,162 | 191/210/221 | 231/250/281 | 80/81/110 | 260/271/301 |
| fixed-11 kernel | 972/1,463/1,463 | 881/1,313/1,433 | 110/210/210 | 220/230/250 | 50/60/70 | 211/240/250 |
| Zstd 1,024 | 46,592/46,873/51,231 | 64,287/64,958/70,409 | 4,670/4,759/4,800 | 15,741/15,841/15,851 | 2,324/2,365/2,394 | 210/220/231 |
| Zstd 2,048 | 46,232/46,692/52,624 | 113,043/126,690/140,056 | 4,629/4,779/4,800 | 28,116/28,196/28,226 | 2,315/2,364/2,375 | 210/220/221 |
| Zstd 4,096 | 46,191/46,753/50,269 | 203,391/208,582/208,802 | 4,649/4,679/4,729 | 51,060/51,381/55,188 | 2,355/2,395/2,435 | 210/221/221 |
| LZ4 1,024 | 27,283/27,394/27,394 | 44,118/44,448/52,323 | 2,756/2,796/2,806 | 9,509/9,699/16,984 | 431/461/471 | 210/230/231 |
| LZ4 2,048 | 27,465/27,504/31,663 | 84,857/88,464/98,103 | 2,766/2,796/2,845 | 19,098/19,128/19,248 | 431/461/481 | 211/241/250 |
| LZ4 4,096 | 24,488/24,539/38,205 | 148,834/153,332/154,695 | 2,485/2,515/2,525 | 35,280/35,460/35,490 | 391/411/411 | 200/211/221 |
| Tabix | 98,045,653/116,889,516/126,684,122 | 147,882,735/163,069,100/164,029,743 | 9,646,423/9,757,101/9,989,760 | 18,867,339/23,291,228/27,812,980 | 9,566,165/9,921,405/9,925,864 | 16,268,786/16,957,764/17,169,672 |

The benchmark emits special-workload logical-byte/page instrumentation for every
mmap candidate using the same sample-level page deduplication as the primary
tables. All special samples had zero minor and major faults. All custom kernels
exactly round-trip the fixture's 6,342 rows, including both complete exception
records.

## Limits

This is comparative warm evidence, not a cold-storage claim. The 9.86-million-
locus lab corpus is smaller than system memory, and serialization warms pages.
Major faults were zero. Definitive cold-I/O, resident-set, and complete
installed-size evidence waits for Ticket 004. The selected reader allocates no
heap proportional to artifact size: fixed metadata lives in the reader value
and the artifact is mapped; result vectors account for query allocations.
