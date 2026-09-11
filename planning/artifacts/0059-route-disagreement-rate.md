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
raw-records: planning/artifacts/0059-route-disagreement-records.tsv
variant-set-size: 2615
compared-records: 2790
value-disagreements: 2
value-disagreement-percent: 0.07
position-compared-records: 379
position-disagreements: 17
position-disagreement-percent: 4.49
both-routes-zero-records: 2409
non-zero-records: 381
non-zero-value-disagreement-percent: 0.52
denominator-composition: 2,409 of the 2,790 compared records score zero on both sides on both routes, so 86 percent of the value denominator is two routes agreeing that nothing happened, and 381 records carry a non-zero score on at least one route.
zero-score-treatment: A gene record enters the position comparison only on a side whose score is non-zero on both routes, and every record both routes answered stays in the value comparison, zero scores included.
substitution-coverage: Every variant in the measured set is a transversion and none of them is a transition, so these figures cover transversions only.
substitution-limit: Splice-site sequence is not base-symmetric, so a transition-bearing set could move either figure by an amount nothing measured.
position-mechanism: PangoPup finds the extremum of the raw 101-value window array and rounds afterwards, and the published dataset appears to round the array to hundredths first and report the first position attaining the winning hundredth. That reduction is inferred from replaying the rule against PangoPup's own arrays, where it reproduces 16 of the 17 observed position disagreements; the upstream software cannot be read from here.
position-ordering: PangoPup breaks a tie to the lowest position and the inferred upstream rule does the same, which would put a precomputed position at or before a modeled one for the same call. That ordering held on all 395 comparable sides of the 379 records measured and none broke it, but it rests on an inferred rule over one set of one substitution class and is an observation, not a guarantee to build on.
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

**What the rule leaves out.** Tying the alternate base to the reference base
through one cycle makes every probe a transversion. `A→C`, `C→G`, `G→T` and
`T→A` each cross the purine/pyrimidine boundary, so the set holds 2,615
transversions and no transitions, while real variant traffic runs about two
transitions to every transversion. The set is also narrowed to positions the
published dataset covers. The 3,549 probed positions that dropped out are the
ones the dataset answers `not_found` at on all four candidates. That
survivorship is the population the contract statement is about, not a distortion
of it. The transversion-only shape is a distortion. Splice-site sequence is not
base-symmetric, so a transition-bearing set could move either figure, and
neither figure should be read as covering transitions.

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

**What the value denominator contains.** The 2,790 compared records are not 2,790
calls the two routes had to agree about. A record that agrees on value renders
the same `gain_score` and the same `loss_score` on both routes, so for an
agreeing record a side is non-zero on both routes exactly when it is non-zero at
all. An agreeing record therefore falls out of the position comparison only when
its gain and its loss are both zero on both routes. Both value disagreements are
outside the position comparison as well, each one a side that is zero on one
route and non-zero on the other. The 2,411 records the position comparison drops
are therefore 2,409 records scored zero on both sides by both routes plus those
two. 86 percent of the value denominator is two routes agreeing that nothing
happened. A consumer gets that agreement for free and must not read it as
agreement about a call. 381 records carry a non-zero score on at least one
route. 379 of those carry one on both routes on the same side. The second
value figure below is measured over those 381.

## Host and build

- Commit `276b40a15e3568b190d14a6f002aca20bf019eb9`, PangoPup 0.5.0, release build.
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
| records scored zero on both sides by both routes | 2409 |
| records carrying a non-zero score on at least one route | 381 |
| records disagreeing on a value, of those 381 | 2 |

The two routes report a different value on **0.07 percent** of the
2790 compared records and a different position on **4.49 percent**
of the 379 records comparable on position.

Read the value figure with its denominator in view. 2,409 of the 2,790 compared
records score zero on both sides on both routes, so 86 percent of that
denominator is two routes agreeing that nothing happened. Over the 381 records
where either route reports a non-zero score, the two routes report a different
value on **0.52 percent**. That is seven times the rate over the full
denominator. Both figures are true and neither replaces the other: 0.07 percent is what a
consumer sees across a stride of the covered genome, and 0.52 percent is what a
consumer sees among the records that carry a call.

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
   hundredths at the bottom of the scale. Most of that agreement is agreement
   about zero: 2,409 of the 2,790 records are `0.00` on both sides on both
   routes. Among the 381 records where either route reports a non-zero score the
   disagreement rate is 0.52 percent, and both disagreements live there.
2. The two routes disagree on position far more often. 17 of the 379 records
   comparable on position carry a different position, and the differences are
   large: `GRCh38:chr6:161000000:A:C` scores `0.47` on both routes and reports
   the gain at `-49` from the published dataset and at `27` from the model. A
   consumer must not compare a precomputed position against a modeled one.
3. The position disagreements look like a reduction rule rather than a defect,
   and that half of the explanation is inferred. PangoPup's own half is read
   from the source: it finds the extremum of the raw 101-value window array and
   rounds afterwards, breaking a tie to the lowest index. The upstream software
   that built the published dataset cannot be read from here, so its rule was
   guessed and tested. Replaying a rule that rounds the array to hundredths
   first and reports the first position attaining the winning hundredth
   reproduces 16 of the 17 observed position disagreements exactly. The
   seventeenth, `GRCh38:chr4:101500000:G:T`, misses the rounding boundary at the
   published position by 1.6e-5 of raw float drift. In every one of the 17 the
   published position is the second- or third-largest value in PangoPup's own
   array, so the two routes read the same array and pick different peaks out of
   it. A rule that reproduces 16 of 17 is a good explanation, not a reading of
   the producer, and no claim here rests on more than that.

   The inferred rule would order the two positions one way: both sides break a
   tie to the lowest index, so a precomputed position would fall at or before a
   modeled one for the same call. That ordering was checked side by side and it
   held on all 395 comparable sides of the 379 comparable records, with no
   exception. It is what 395 sides of one transversion-only set on one build
   showed, resting on a rule nobody here can read. Do not build on it as a
   guarantee: it is bounded by the evidence in this artifact, and a
   transition-bearing set or a later dataset could break it.
4. Most of the set is not comparable on position at all. 2,411 of the 2,790
   compared records fall out of the position comparison, and 2,409 of those are
   zero on both sides on both routes rather than zero on one side of one route.
   That is the 0040 result restated on this set: a position beside a zero score
   is not a position a consumer reads.
5. The rate is low enough that it changes no guidance a consumer follows. The
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

```
# 3. Write one row per compared gene record.
#    variant, stable_gene, then the four scores and four positions in the order
#    bundle gain, bundle gain position, bundle loss, bundle loss position,
#    model gain, model gain position, model loss, model loss position,
#    tab-separated under that exact header. Every position is an integer on
#    every row, including a row whose score beside it is zero, so nothing is
#    blanked. A bundle position of -50 beside a zero bundle score is the
#    published dataset's sentinel, and every one of the 5,184 zero bundle sides
#    here carries it. A model position beside a zero model score is not a
#    sentinel at all: it is wherever the model's own extremum fell, and it lands
#    on -50 for 2,467 of the 5,184 zero model sides and on one of the other 100
#    positions for the rest. Read no position beside a zero score on either
#    route.
```

That file is `planning/artifacts/0059-route-disagreement-records.tsv`. It holds
all 2790 compared records, not a sample of them, and every count in the
`measurement` block above is recomputed from it by
`tests/route-disagreement-rate.sh` rather than read from this prose.

Run on 2026-09-10 against a scratch copy of the installed runtime:

```
2615 variants scored through the model in 14 concurrent processes,
33 minutes 1 second of wall clock, 2,790 gene records joined,
2 value disagreements and 17 position disagreements.
```

No gate re-runs any of this. `tests/route-disagreement-rate.sh` holds the
published statement to the block above, to the manifest and to the per-record
file, and nothing more.

The run recorded here is the second. The first, on the same set and the same
assets, produced the same nine counts and the same 19 disagreeing records. Both
routes are deterministic on one host and one build, so a third run reproduces
them again.
