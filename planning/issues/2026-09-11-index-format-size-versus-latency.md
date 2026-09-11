# The index is 14 GiB because size was ranked third

Status: open

## Observation

`scores.pgi` is 15,030,604,105 bytes. A measured alternative holds the same data
in 1,706,199,888 bytes and answers from mmap without decompressing anything. The
difference is 13,324,404,217 bytes, and the measured latency cost is 39
nanoseconds on the comparison that was run.

Nothing about the data requires 14 GiB. ADR 0004 ranked query performance first,
resident memory second, and size third, and ADR 0006 selected the fixed layout
under that ranking. The ranking is the reason, not the format.

## What the 11 bytes hold

One locus stores three alternate records plus the reference base. The reference
implies which three alternates exist, so the alternates are not stored. Each
alternate record holds four values: gain score, gain position offset, loss
magnitude, and loss position offset. Each value has 101 possibilities and
occupies seven bits.

Four values at seven bits is 28 bits per alternate. Three alternates is 84 bits.
Three bits for the reference base gives 87 bits, rounded up to 11 bytes. One bit
is unused.

The position offsets are half the payload. A caller needs to know where the
predicted site sits, not only how strong the prediction is.

## What the data actually carries

[`planning/artifacts/2026-07-20-full-dataset-entropy.md`](../artifacts/2026-07-20-full-dataset-entropy.md)
streamed all 4,099,255,665 source rows and measured the distribution.

| Measure | Result |
|---|---:|
| Empirical zero-order entropy, joint locus symbol | 5.995913 bits |
| Corpus at that entropy | 1,024,115,911 bytes |
| Stored per locus today | 87 bits |

The format is 14.5 times larger than its own zero-order entropy. The reason is
concentration:

| Observation | Fraction |
|---|---:|
| Loci where all six score/position pairs are default | 77.293% |
| Records equal to the exact default record | 87.143% |
| Records equal to the preceding locus's same-alt record | 80.609% |
| Loss score is zero | 98.799% |
| Corpus covered by the top 256 distinct records | 95.947% |

A fixed-width array pays full price for 1,056,149,297 loci that say nothing.

## The measured alternative

The hierarchical sparse direct layout stores two reference bits per locus, one
presence bit per locus, a six-bit pair mask for the 310,269,258 loci that carry
anything, a 14-bit value for each of the 549,194,849 nondefault pairs, and rank
checkpoints every 64 loci inside 4,096-locus blocks. Lookup uses bounded
popcounts over mapped bytes. No block is decompressed.

| Format | Bytes | GiB |
|---|---:|---:|
| Fixed 11-byte, shipping | 15,030,604,105 | 13.998 |
| Hierarchical sparse direct, calculated | 1,706,199,888 | 1.589 |
| Zstd-1 blocks at 4,096 loci | 1,617,984,690 | 1.507 |
| Joint-locus entropy floor | 1,024,115,911 | 0.954 |

Ticket 002 benchmarked the first two on an equal candidate harness. Warm p50 at
1, 10 and 100 lookups:

| Reader | 1 | 10 | 100 |
|---|---:|---:|---:|
| Fixed candidate | 121 ns | 972 ns | 9,949 ns |
| Sparse direct candidate | 160 ns | 1,243 ns | 14,749 ns |
| Fixed hardened product reader | 210 ns | 1,964 ns | 19,588 ns |

## What this costs to reconsider

Three facts make this larger than a configuration change.

The 1,706,199,888-byte figure is calculated rather than built. It excludes the
rank, gene, segment and provenance directories, and it excludes the `REF=N`
exception table.

The sparse candidate was never hardened. The fixed reader went from 121 ns to
210 ns when manifest validation, segment and interval-tree checking, and
six-pair validation were added. The equivalent overhead on a sparse reader has
not been measured, so the shipped-path delta is unknown.

ADR 0006 removed the comparison implementation. The code would be written again.

A format change also mints a new certified private version, a re-hardened
reader, and a re-released asset.

## What changes the answer

The compressed download is already 2.44 GiB, so this is not a download argument.
The 13.3 GB is resident footprint and page-cache pressure. The case for
reopening rests on running many instances, on container image size, and on hosts
where 14 GiB of page cache is not available.

Ian stated on 2026-09-11 that he would trade ten to one hundred times the
current lookup latency for a materially smaller index. The measured trade is
1.3 times on the candidate harness. That is two orders of magnitude inside his
stated tolerance, and it contradicts the ranking ADR 0004 recorded.

## What a ticket would need

Per [`planning/README.md`](../README.md), file-format work must state the size
and performance evidence that can accept or reject it.

- A decision from Ian on the ADR 0004 optimization order, recorded as a
  superseding ADR. Everything else depends on it.
- The built sparse payload size including all directories and the exception
  table, against the calculated 1,706,199,888.
- The hardened sparse reader latency at 1, 10 and 100 lookups, against the
  hardened fixed reader's 210 / 1,964 / 19,588 ns.
- The stated acceptance threshold, decided before the numbers are seen.
- The migration and version story, because a shipped bundle identity changes.
