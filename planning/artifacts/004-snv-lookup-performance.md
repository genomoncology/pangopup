# Ticket 004 SNV lookup performance and full-corpus oracle

Date: 2026-07-22

This report binds the Ticket 004 lookup oracle and warm performance measurements
to a fresh, independently verified production bundle. The generated 15 GB bundle
was deleted after the retained evidence was complete.

## Build and data identity

| Field | Value |
|---|---|
| Base Git commit | `38808f5f419446aa973d13f96fdedc700c3595c5` plus the uncommitted Ticket 004 implementation under test |
| Builder version / source | `0.1.0` / `sha256:d566f7405478fcd72dbbef0f10abd58da3d381127fbbedebf08e049a57427ce2` |
| Compiler | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, LLVM 21.1.8, `x86_64-unknown-linux-gnu` |
| OS | Ubuntu Linux, kernel `6.17.0-35-generic`, x86-64 |
| Host | AMD Ryzen 7 5825U, 8 cores / 16 threads; 29,340,872,704 bytes RAM; Crucial CT1000P3PSSD8 NVMe |
| Source members | 19,913; `sha256:0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6` |
| Published source archive | 12,988,141,317 bytes; `md5:679ef0b50e511b6102b4b88fbf811108` |
| Reference input | 972,898,531 bytes; `sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3` |
| Canonical 25-sequence set | `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4` |
| `scores.pgi` | 15,033,158,255 bytes; `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27` |
| Bundle ID | `sha256:3a5d7de6aacf2aada1ff327764e21d5142ad8a534f7f861fb127576a664d5ee2` |
| Query manifest | `planning/artifacts/004-query-manifest.tsv`; `sha256:36644941adbf78419ff9cf5c42ae57e46cca336b1099f9c9e9902d0b30ea8cfa` |
| Source-derived oracle | `planning/artifacts/004-full-oracle.jsonl`; `sha256:c93c75bbf61b39f7fd88c868b2fe01eb29117e24c6c09df565275cc709a05119` |
| Retained extractor | `planning/artifacts/004-source-oracle.py`; `sha256:b9a2224f2cc1f1d58fc1e74be45d42ba9fdd896354a764793daf57b4cad82675` |

The accepted bundle has 19,913 genes, 4,099,255,665 source rows,
1,366,418,555 gene loci, 10,073 ascending and 9,840 descending members,
19,916 source segments, 19,945 index segments, three gap transitions, and
50,002 omitted bases. Its 30 `REF=N` loci comprise nine omit-A and 21 omit-T
shapes. The canonical source and decoded streams both contain 4,099,255,665
records and both hash to
`sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31`.
The independently invoked verifier returned `verified`, the same bundle ID, and
two verified members.

The exact acceptance commands were:

```bash
cargo build --locked --release -p pangopup-build -p pangopup-cli
target/release/pangopup-build build \
  --source "$PANGOPUP_SOURCE_DIR" \
  --reference "$PANGOPUP_GRCH38_FASTA" \
  --output "$RUN/bundle"
target/release/pangopup-build verify "$RUN/bundle"
uv run --script planning/artifacts/004-source-oracle.py \
  planning/artifacts/004-query-manifest.tsv \
  "$PANGOPUP_SOURCE_DIR" >"$RUN/oracle.jsonl"
cmp planning/artifacts/004-full-oracle.jsonl "$RUN/oracle.jsonl"
```

The extractor reads every one of the 19,913 gzip members. It derives exact
centi-score text from decimal source fields, constructs `REF=N` ambiguity
shapes, and establishes misses by complete relevant-source inspection. A
separate validator grouped the manifest into 13 one-open CLI invocations and
compared all 260 CLI outcomes, after removing bundle provenance only, with the
source oracle. All 260 matched.

The manifest has exact 1/10/100 filtered and unfiltered primary batches, plus
same-page, true-overlap, cross-contig, ambiguity, random-hit, and random-miss
stress cases. Seeded samples use unsigned 32-bit LCG state
`state = 1664525 * state + 1013904223`, seed `0x50414e47`, and take the first
ten unique `state % 100` values: `86,93,0,67,82,61,84,95,74,33`. Hits index the
100-hit primary universe; misses index positions 1 through 100 on chr1 with
the fixed `A>C` allele. Complete source inspection proves the selected misses.

## Benchmark method

The benchmark was built and run as follows; the executable suffix is Cargo's
content-derived harness name for this build:

```bash
cargo build --locked --release -p pangopup-cli --bench snv_lookup
PANGOPUP_BUNDLE="$RUN/bundle" \
PANGOPUP_QUERY_MANIFEST="$PWD/planning/artifacts/004-query-manifest.tsv" \
PANGOPUP_CLI="$PWD/target/release/pangopup" \
  target/release/deps/snv_lookup-1aa28366c6ba4c05 >"$RUN/benchmark.tsv"
```

Every measurement has 20 unretained warmups followed by 100 retained samples.
p50/p95/p99 are nearest-rank values from sorted retained samples. These are
warm page-cache results: both the 15,033,163,553-byte bundle and available
29,340,872,704-byte memory fit on this host. No isolated device or OS method
proved pages nonresident, so cold performance is deliberately **unmeasured**.

`fresh-cli` is one new real release CLI child for each complete batch, including
parse, open, lookup, serialization, and a captured stdout pipe. Its faults are
the sum over 100 retained children and RSS is the largest GNU `time` peak.
`open-only` creates a new in-process provider each sample. `lookup-only` reuses
one provider. `serialization-*` materializes results before timing and renders
to a memory buffer through the same public renderer called by the shipped CLI;
the harness compares every warmup and retained fresh-child stdout byte-for-byte
with that renderer before accepting a sample. Across 13 workloads, this made
1,560 exact stdout comparisons (20 warmups plus 100 retained samples each).
The in-process sampler allocates and initializes all timing storage before its
warmups, allocation-counter reset, and fault/RSS baseline. An empty-operation
runtime regression check must report exactly zero allocation calls and bytes
before any measurements are accepted.
In-process allocation counts/bytes are tracking-allocator observations per
batch; child allocations are unavailable and shown as N/A.
In-process faults and peak-RSS differences cover the 100 retained samples.
Logical bytes and pages are algorithm-addressed index ranges, using a fixed
4 KiB page number; they are not claims about storage reads or residency.

## Direct answer: open and return 1, 10, or 100 scores

Warm open-only cost was **1,167.643 us p50**, 1,242.390 us p95, and
1,280.644 us p99. The complete fresh CLI returned filtered batches of 1, 10,
and 100 distinct score records in **2,566.029 us**, **2,566.490 us**, and
**2,897.891 us** p50 respectively. With an already-open provider, lookup-only
cost was **0.441 us**, **5.521 us**, and **44.628 us** p50. Unfiltered batches
returning the same counts were 2,548.315 / 2,530.109 / 2,969.973 us in a fresh
CLI and 0.602 / 5.932 / 53.034 us through the already-open provider. These
numbers cover exact precomputed score retrieval only; they make no HTTP,
model-inference, or cold-cache claim.

## Primary fresh CLI results

Faults are retained-run totals; RSS is peak KiB and output is bytes per batch.

| Workload | Requests / records | p50 / p95 / p99 us | Batches/s | Records/s | Minor / major faults | Peak RSS KiB | Output bytes |
|---|---:|---:|---:|---:|---:|---:|---:|
| filtered-1 | 1 / 1 | 2566.029 / 2777.775 / 2857.882 | 389.707 | 389.707 | 14531 / 0 | 4984 | 492 |
| filtered-10 | 10 / 10 | 2566.490 / 2762.535 / 2889.714 | 389.637 | 3896.372 | 14632 / 0 | 4984 | 4920 |
| filtered-100 | 100 / 100 | 2897.891 / 3089.177 / 3192.640 | 345.079 | 34507.854 | 17061 / 0 | 5112 | 49176 |
| unfiltered-1 | 1 / 1 | 2548.315 / 2769.068 / 2847.132 | 392.416 | 392.416 | 14628 / 0 | 4984 | 492 |
| unfiltered-10 | 10 / 10 | 2530.109 / 2794.899 / 2865.798 | 395.240 | 3952.399 | 14636 / 0 | 4984 | 4920 |
| unfiltered-100 | 100 / 100 | 2969.973 / 3246.897 / 3407.541 | 336.703 | 33670.340 | 17063 / 0 | 5112 | 49176 |

## Primary already-open lookup results

Allocations are calls / bytes per complete batch. Fault and RSS deltas were
zero for every row.

| Workload | Requests / records | p50 / p95 / p99 us | Batches/s | Records/s | Allocations | Logical bytes / pages |
|---|---:|---:|---:|---:|---:|---:|
| filtered-1 | 1 / 1 | 0.441 / 0.481 / 0.531 | 2267573.696 | 2267573.696 | 8.00 / 222 | 1707 / 11 |
| filtered-10 | 10 / 10 | 5.521 / 5.641 / 5.711 | 181126.607 | 1811266.075 | 80.00 / 2220 | 17070 / 11 |
| filtered-100 | 100 / 100 | 44.628 / 56.150 / 61.220 | 22407.457 | 2240745.720 | 800.00 / 22200 | 170700 / 11 |
| unfiltered-1 | 1 / 1 | 0.602 / 0.641 / 0.681 | 1661129.568 | 1661129.568 | 8.00 / 222 | 1739 / 12 |
| unfiltered-10 | 10 / 10 | 5.932 / 6.052 / 6.072 | 168577.208 | 1685772.084 | 80.00 / 2220 | 17390 / 12 |
| unfiltered-100 | 100 / 100 | 53.034 / 64.317 / 71.280 | 18855.828 | 1885582.834 | 800.00 / 22200 | 173900 / 12 |

Open-only allocated 1,455.00 calls / 66,136 bytes per open and incurred 4,000
minor and zero major faults over retained samples. Its observed peak-RSS delta
was zero.

## Primary serialization-only results

| Format / workload | Requests / records | p50 / p95 / p99 us | Allocations calls / bytes | Output bytes |
|---|---:|---:|---:|---:|
| JSONL filtered-1 | 1 / 1 | 0.762 / 0.792 / 0.802 | 15.00 / 1160 | 492 |
| JSONL filtered-10 | 10 / 10 | 7.795 / 7.915 / 8.015 | 91.00 / 17816 | 4920 |
| JSONL filtered-100 | 100 / 100 | 77.442 / 97.241 / 97.752 | 814.00 / 145464 | 49176 |
| table filtered-1 | 1 / 1 | 0.400 / 0.411 / 0.440 | 8.00 / 931 | 272 |
| table filtered-10 | 10 / 10 | 3.767 / 3.818 / 3.838 | 65.00 / 9518 | 1568 |
| table filtered-100 | 100 / 100 | 37.564 / 38.135 / 40.970 | 608.00 / 87772 | 14504 |
| JSONL unfiltered-1 | 1 / 1 | 0.962 / 1.022 / 1.042 | 15.00 / 1160 | 492 |
| JSONL unfiltered-10 | 10 / 10 | 7.114 / 7.244 / 7.284 | 91.00 / 17816 | 4920 |
| JSONL unfiltered-100 | 100 / 100 | 77.853 / 81.219 / 83.304 | 814.00 / 145464 | 49176 |
| table unfiltered-1 | 1 / 1 | 0.511 / 0.531 / 0.541 | 8.00 / 931 | 272 |
| table unfiltered-10 | 10 / 10 | 3.737 / 3.797 / 3.848 | 65.00 / 9518 | 1568 |
| table unfiltered-100 | 100 / 100 | 40.159 / 41.332 / 45.169 | 608.00 / 87772 | 14504 |

All primary serialization rows had zero retained fault and RSS deltas.

## Stress results

The stress table reports the already-open lookup path. All retained fault and
RSS deltas were zero.

| Workload | Requests / records | p50 / p95 / p99 us | Allocations calls / bytes | Logical bytes / pages |
|---|---:|---:|---:|---:|
| same-page | 10 / 10 | 5.501 / 5.601 / 5.681 | 80.00 / 2220 | 17070 / 11 |
| seeded random hits | 10 / 10 | 5.971 / 6.101 / 6.173 | 80.00 / 2220 | 17390 / 12 |
| seeded random misses | 10 / 0 | 5.591 / 5.711 / 5.751 | 90.00 / 1740 | 21280 / 12 |
| true overlap | 1 / 2 | 0.791 / 0.822 / 0.872 | 10.00 / 238 | 1990 / 14 |
| cross-contig | 5 / 6 | 3.657 / 3.747 / 3.788 | 50.00 / 1190 | 9106 / 59 |
| ambiguity omit-A | 1 / 0 | 0.631 / 0.691 / 0.702 | 10.00 / 238 | 1680 / 10 |
| ambiguity omit-T | 1 / 0 | 0.641 / 0.672 / 0.691 | 10.00 / 238 | 1680 / 10 |

The fresh CLI stress results retain the same complete-process boundary as the
primary table:

| Workload | Requests / records | p50 / p95 / p99 us | Minor / major faults | Peak RSS KiB | Output bytes |
|---|---:|---:|---:|---:|---:|
| same-page | 10 / 10 | 2631.077 / 2834.907 / 2945.284 | 14634 / 0 | 4984 | 4920 |
| seeded random hits | 10 / 10 | 2567.713 / 2730.432 / 2751.654 | 14640 / 0 | 4984 | 4917 |
| seeded random misses | 10 / 0 | 2594.155 / 2820.769 / 2884.354 | 14530 / 0 | 4868 | 3829 |
| true overlap | 1 / 2 | 2549.186 / 2754.299 / 2828.725 | 14723 / 0 | 5024 | 597 |
| cross-contig | 5 / 6 | 2617.240 / 2834.416 / 2969.361 | 15031 / 0 | 5304 | 2566 |
| ambiguity omit-A | 1 / 0 | 2567.963 / 2770.661 / 2824.687 | 14429 / 0 | 4868 | 500 |
| ambiguity omit-T | 1 / 0 | 2567.573 / 2780.010 / 2809.898 | 14428 / 0 | 4864 | 500 |

Serialization-only stress measurements show the formatting contribution after
materialization. All had zero retained fault and RSS deltas.

| Format / workload | Requests / records | p50 / p95 / p99 us | Allocations calls / bytes | Output bytes |
|---|---:|---:|---:|---:|
| JSONL same-page | 10 / 10 | 6.973 / 8.707 / 8.728 | 91.00 / 17816 | 4920 |
| table same-page | 10 / 10 | 3.747 / 3.797 / 3.817 | 65.00 / 9518 | 1568 |
| JSONL seeded hits | 10 / 10 | 6.964 / 8.788 / 8.927 | 91.00 / 17816 | 4917 |
| table seeded hits | 10 / 10 | 3.827 / 3.878 / 3.898 | 65.00 / 9518 | 1565 |
| JSONL seeded misses | 10 / 0 | 4.329 / 4.408 / 4.419 | 40.00 / 8424 | 3829 |
| table seeded misses | 10 / 0 | 2.064 / 2.104 / 2.114 | 55.00 / 7148 | 1297 |
| JSONL overlap | 1 / 2 | 1.293 / 1.383 / 1.422 | 20.00 / 2304 | 597 |
| table overlap | 1 / 2 | 0.832 / 0.852 / 0.902 | 13.00 / 1904 | 413 |
| JSONL cross-contig | 5 / 6 | 3.988 / 4.068 / 4.118 | 54.00 / 9024 | 2566 |
| table cross-contig | 5 / 6 | 2.244 / 2.284 / 2.304 | 38.00 / 5145 | 990 |
| JSONL ambiguity omit-A | 1 / 0 | 1.032 / 1.082 / 1.202 | 17.00 / 1232 | 500 |
| table ambiguity omit-A | 1 / 0 | 0.551 / 0.571 / 0.601 | 12.00 / 970 | 288 |
| JSONL ambiguity omit-T | 1 / 0 | 1.032 / 1.062 / 1.102 | 17.00 / 1232 | 500 |
| table ambiguity omit-T | 1 / 0 | 0.531 / 0.542 / 0.592 | 12.00 / 970 | 288 |

## Deviation and disposition

An initial full build accidentally invoked the pre-Ticket-004 release builder.
It reproduced the pinned `scores.pgi` hash but embedded the older builder-source
digest, so its bundle identity was rejected and none of its oracle or benchmark
measurements were accepted. The release builder was explicitly rebuilt from
the Ticket 004 source, a small build confirmed the new source digest, and the
entire production build, independent verify, oracle validation, and benchmark
were repeated against the accepted identity above. The pinned index hash did
not change, so no format ADR or writer change was needed.

The code-review remediation then moved rendering behind the CLI library's
single public renderer, made opened-bundle internals private and provenance
frozen, validated contig syntax before bundle I/O, and bounded manifest reads
before allocation. Those source changes altered the builder-source digest and
therefore required the complete production build, independent verification,
CLI oracle comparison, and benchmark to be recertified again. The identities
and measurements throughout this report are from that final remediation build.
The retained query manifest, source archive, reference, index bytes, and
source-derived oracle were unchanged, so the expensive source extractor was
not rerun merely to reproduce identical retained bytes; the 260-outcome CLI
comparison against that oracle was rerun against the final bundle and passed.

A final adversarial re-review found that the sampler allocated its 100-element
timing vector after resetting the allocation counters and taking the resource
baseline. That harness-only vector had overstated every prior lookup and
serialization result by exactly 0.01 allocation calls and 16 bytes per batch,
and could also contaminate fault/RSS deltas. The sampler now allocates and
touches that storage before warmup and both baselines, and an empty-operation
check proves zero allocations are counted. The complete harness was rebuilt
(`sha256:7af48e830cf23e06e4c73ef4b2cda82450741da3555a7d083a4e982e902a610c`)
and rerun; all lookup and serialization allocation rows now contain the exact
operation-only integer call counts and 16 fewer bytes. Open-only changed from
1,455.01 calls / 66,020 bytes to 1,455.00 / 66,136; its allocation bytes also
depend on the bundle path, and this recertification used a longer path, so only
the removed 0.01 harness call is directly comparable. The rebuilt bundle again
had builder digest
`sha256:d566f7405478fcd72dbbef0f10abd58da3d381127fbbedebf08e049a57427ce2`,
the same pinned index hash and bundle ID, and passed a new standalone verify.
