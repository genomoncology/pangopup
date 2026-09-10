# Ticket 0059 — how often the two scoring routes disagree

Date: 2026-09-10

## Question

`architecture/compatibility.md` told a consumer that a precomputed score and a
modeled score are not interchangeable and then declined to say how often they
differ. The only evidence behind the warning was the frozen `pangolin-compat-v1`
corpus: four variants, five gene records, one disagreement. Five records
establish no rate. This run measures one, over a named set, against the shipped
v0.5.0 assets.

```measurement
variant-set: snv-stride-500k-v1
variant-set-rule: Every position that is a multiple of 500,000 on chr1 through chr22, chrX and chrY, kept when the published SNV dataset answers one of the four candidate SNVs built there.
variant-set-manifest: planning/artifacts/0059-route-disagreement-set.tsv
variant-set-size: 2615
compared-records: 2790
value-disagreements: 2
value-disagreement-percent: 0.07
position-compared-records: 379
position-disagreements: 17
position-disagreement-percent: 4.49
zero-score-treatment: A gene record enters the position comparison only on a side whose score is non-zero on both routes, and every record both routes answered stays in the value comparison, zero scores included.
evidence-limit: This rate was measured once on one host against the shipped v0.5.0 assets and no gate re-runs it.
measured: 2026-09-10
```

## Method

**The variant set.** The set is drawn by a rule over the shipped SNV dataset, not
picked by hand. Walk chr1 through chr22, chrX and chrY in that order. On each
contig take every position that is a multiple of 500,000 and does not exceed the
contig length the shipped executable reports, which the executable states in its
own refusal for a position past the end. At each position build four candidate
SNVs, one per reference base `A`, `C`, `G`, `T`, each carrying the next base in
the cycle `A→C→G→T→A` as its alternate. Submit all four to the published dataset
alone with `pangopup lookup --bundle`, which routes to no model. The dataset
answers `found` for the candidate whose `REF` matches its published source
reference and `not_found` for the other three, so the probe both selects the
covered positions and recovers the reference base without reading the reference
bundle. 6,164 positions were probed and 2615 of them survived. Those
2615 variants are `planning/artifacts/0059-route-disagreement-set.tsv`, in
contig order and then ascending position.

The rule names no gene and no variant. A later reader who walks the same stride
over the same bundle gets the same file.

**The two routes.** The published runtime installation was copied to a scratch
data root and every run was pointed at the copy with `--data-dir`. The
installation itself was never written to, and every run used a private model
result cache under the scratch root. The precomputed answer is the `--bundle`
probe above. The modeled answer is `pangopup lookup --data-dir <copy>
--model-only` over the same 2615 variants, split into 14 concurrent
processes with a private cache each.

**What is compared.** A gene record is joined across the routes on the variant
and the `stable_gene` field, because the modeled route reports a versioned
GENCODE identity and the precomputed route reports the stable one. A record both
routes answered is a compared record.

*Value.* A compared record disagrees on value when the two routes render a
different `gain_score` or a different `loss_score`. Every compared record counts,
zero scores included: a zero is a value, and disagreeing about it is
disagreement.

*Position.* Ticket 0040 settled that a position is readable only where the score
beside it is non-zero, so a comparison of two positions needs both scores
non-zero. A compared record's gain side is comparable when both routes score the
gain non-zero, and its loss side is comparable when both routes score the loss
non-zero. A record with at least one comparable side is a position-compared
record, and it disagrees when a comparable side carries a different position on
the two routes. A record with no comparable side is left out of the position
denominator and stays in the value denominator. The gap between the two
denominators — 2,411 records — is what the zero scores cost the position
measurement.

## Host and build

- Commit `d78e948009045bc4bcf5b490e4674b80110f44de`, PangoPup 0.5.0, release build.
- AMD Ryzen 7 5825U, 16 logical CPUs, Linux 6.17.0-35-generic x86_64.
- SNV bundle `sha256:c4c4162b34a73ecd8c44d379f9e4fbc4e5e07869af1967a6695b8d439d2819b3`.
- Model bundle `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`.
- Reference bundle `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`.
- Mask `sha256:714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.
- Model profile `pangolin-1.0.2-5cf94b8-onnx-cpu-v1`, effective CPU policy `sequential:1/1`.

## Result

| measure | count |
| --- | --- |
| variants in the set | 2615 |
| variants both routes answered | 2615 |
| gene records both routes answered | 2790 |
| records disagreeing on a value | 2 |
| records comparable on position | 379 |
| records disagreeing on a comparable position | 17 |

The two routes report a different value on **0.07 percent** of the
2790 compared records and a different position on **4.49 percent**
of the 379 records comparable on position.

Value disagreement is rare and position disagreement is not. The two figures are
two orders of magnitude apart, which is what the existing warning meant when it
said positions diverge more widely than values. It now has a number.

## The disagreeing records

| variant | gene | gain precomputed / model | loss precomputed / model | disagreement |
| --- | --- | --- | --- | --- |
| `GRCh38:chr1:3500000:C:G` | ENSG00000162591 | 0.02 at 10 / 0.02 at 45 | 0.00 at -50 / 0.00 at -49 | position |
| `GRCh38:chr10:50500000:C:G` | ENSG00000198964 | 0.01 at -3 / 0.01 at 2 | 0.00 at -50 / 0.00 at -49 | position |
| `GRCh38:chr10:95500000:C:G` | ENSG00000095637 | 0.01 at -13 / 0.01 at 49 | 0.00 at -50 / 0.00 at -49 | position |
| `GRCh38:chr10:103500000:G:T` | ENSG00000107954 | 0.01 at -21 / 0.01 at 37 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr11:121500000:T:A` | ENSG00000137642 | 0.01 at -9 / 0.01 at 23 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr14:31000000:C:G` | ENSG00000196792 | 0.01 at 0 / 0.01 at 38 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr14:35000000:T:A` | ENSG00000100883 | 0.00 at -50 / 0.01 at -2 | 0.00 at -50 / 0.00 at -50 | value |
| `GRCh38:chr15:49000000:A:C` | ENSG00000138593 | 0.01 at -46 / 0.01 at -9 | -0.07 at -13 / -0.07 at -13 | position |
| `GRCh38:chr17:39500000:C:G` | ENSG00000167258 | 0.01 at -30 / 0.01 at -1 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr17:58000000:C:G` | ENSG00000266086 | 0.01 at -27 / 0.01 at 39 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr19:51000000:G:T` | ENSG00000269741 | 0.09 at -4 / 0.00 at 5 | 0.00 at -50 / 0.00 at -50 | value |
| `GRCh38:chr2:143000000:A:C` | ENSG00000115919 | 0.01 at -48 / 0.01 at 16 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr3:149500000:T:A` | ENSG00000169903 | 0.01 at -36 / 0.01 at 49 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr4:57000000:T:A` | ENSG00000047315 | 0.03 at -40 / 0.03 at 27 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr4:101500000:G:T` | ENSG00000153064 | 0.01 at -40 / 0.01 at 22 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr4:151000000:C:G` | ENSG00000198589 | 0.01 at -1 / 0.01 at 0 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr6:161000000:A:C` | ENSG00000085511 | 0.47 at -49 / 0.47 at 27 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chr7:71500000:T:A` | ENSG00000185274 | 0.06 at -38 / 0.06 at 18 | 0.00 at -50 / 0.00 at -50 | position |
| `GRCh38:chrX:101500000:C:G` | ENSG00000196440 | 0.01 at 0 / 0.01 at 43 | 0.00 at -50 / 0.00 at -50 | position |

## Findings

1. The two routes agree on the value of a covered SNV almost always. Two gene
   records out of 2,790 disagree, one where the published dataset reports `0.00`
   and the model reports `0.01`, and one where the published dataset reports
   `0.09` and the model reports `0.00`. Both differences are one or two
   hundredths at the bottom of the scale.
2. The two routes disagree on position far more often. 17 of the 379 records
   comparable on position carry a different position, and the differences are
   large: `GRCh38:chr6:161000000:A:C` scores `0.47` on both routes and reports
   the gain at `-49` from the published dataset and at `27` from the model. A
   consumer must not compare a precomputed position against a modeled one.
3. Most of the set is not comparable on position at all. 2,411 of the 2,790
   compared records score zero on at least one side of one route, which is the
   0040 result restated on this set: a position beside a zero score is not a
   position a consumer reads.
4. The rate is low enough that it changes no guidance a consumer follows. The
   contract already says a consumer must record which route answered and must
   not treat the two scores as one measurement. Nothing here weakens or
   strengthens that, so this ticket files no successor.

## Reproducing

The measurement is two commands and a comparison. Both read the shipped v0.5.0
assets and neither writes to the installed runtime.

```
# 1. Draw the set. Every 500,000th position on chr1..chr22,chrX,chrY, four
#    candidate REF bases each, kept where the published dataset answers.
pangopup lookup --bundle <SNV bundle>/bundle --format jsonl \
  --variant GRCh38:chr1:500000:A:C --variant GRCh38:chr1:500000:C:G ... \
  | grep '"status":"found"'

# 2. Score the surviving variants through the model alone.
pangopup lookup --data-dir <scratch copy of the installed runtime> --model-only \
  --model-cache <private cache> --format jsonl \
  --variant <each line of planning/artifacts/0059-route-disagreement-set.tsv>
```

Join the two JSONL streams on the variant and `stable_gene`, then count as
`## Method` describes. The manifest beside this file is the set the run above
produced, so step 2 alone reproduces the numbers from a committed input.

Run on 2026-09-10 against a scratch copy of the installed runtime:

```
2615 variants scored through the model in 14 concurrent processes,
34 minutes 30 seconds of wall clock, 2,790 gene records joined,
2 value disagreements and 17 position disagreements.
```

No gate re-runs any of this. `tests/route-disagreement-rate.sh` holds the
published statement to the block above and to the manifest, and nothing more.
