# What a score value is

A PangoPup score is an exact decimal in hundredths. A response carries that
decimal and never a floating-point value. This file states the value space, the
rounding that produces it, the rendered form, what a consumer can compare it
against, and what moves it.

## The value space

A score is carried in hundredths. `gain_score` takes 101 values, `0.00` through
`1.00`. `loss_score` takes the same 101 magnitudes with the sign restored,
`0.00` through `-1.00`. No value lies between two neighbouring hundredths.

A rendered score always carries exactly two decimal places and one digit before
the decimal point. A zero loss renders `0.00` and never `-0.00`.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-core \
  --test score_value a_score_value_is_one_of_one_hundred_and_one_exact_hundredths >/dev/null
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-core \
  --test score_value a_loss_carries_the_same_value_space_with_its_sign_restored >/dev/null
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  | rg -o '"gain_score":"[^"]*","gain_position":-?[0-9]+,"loss_score":"[^"]*"' \
  | mustmatch like '"gain_score":"0.00","gain_position":-50,"loss_score":"0.00"'
printf 'a score value is one of 101 exact hundredths\n' | mustmatch like 'a score value is one of 101 exact hundredths'
```

## Rounding

The model produces a floating-point value. PangoPup multiplies it by 100, rounds
half to even, and reports the resulting hundredth. A tie does not always round
up.

A model value of 0.105 reports `0.10`. A model value of 0.115 reports `0.12`.
Half-up would report `0.11` and `0.12`. That pair tells the two rules apart.

The precomputed route does not round. It reads exact hundredths from the
published dataset and refuses a source value it cannot carry.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-engine \
  --lib a_halfway_model_value_rounds_to_the_even_hundredth >/dev/null
printf 'a halfway model value rounds to the even hundredth\n' | mustmatch like 'a halfway model value rounds to the even hundredth'
```

## Comparing a score against a threshold

A threshold finer than one hundredth cannot be evaluated against a PangoPup
score. The representable neighbours of 0.106 are `0.10` and `0.11`, and no
PangoPup response distinguishes them. A rule written against 0.106 is a rule
against one of those two values. The deployment must record which one it
compared against.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-core \
  --test score_value a_threshold_finer_than_one_hundredth_has_no_value_to_compare_against >/dev/null
pangopup lookup \
  --model-only \
  --variant GRCh38:chr1:5051:A:C \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"gain_score":"[01]\.[0-9]{2}"' \
  | mustmatch like '"gain_score":"0.33"'
printf 'a threshold finer than one hundredth has no value to compare against\n' | mustmatch like 'a threshold finer than one hundredth has no value to compare against'
```

## The two routes

A covered SNV is answered from the published dataset unless the request asks for
the model. A supported non-SNV, a supported lookup miss and an explicit
`--model-only` request each run the model. The two routes do not always report
the same value for the same variant.

The frozen upstream corpus carries both routes for four variants and five gene
records. Four records agree on the value. One does not.
`GRCh38:chr10:114306065:A:T` reports `0.06` at position 12 from the published
dataset and `0.02` at position 13 from the model. A consumer must not treat a
precomputed score and a modeled score as the same measurement. Five gene records
cannot establish a rate, so both routes were run over a larger set. They report a
different value on 0.07 percent of the 2,790 gene records both answered and a
different position on 4.49 percent of the 379 records comparable on position.
[`planning/artifacts/0059-route-disagreement-rate.md`](../planning/artifacts/0059-route-disagreement-rate.md)
records that measurement, the set it was drawn over, and how zero scores were
treated.

Positions diverge more widely than values. A zero score carries no meaningful
position on either route. The published dataset reports `-50` beside almost every
zero score and reports some other position for a small remainder. A scan of the
whole shipped v0.5.0 SNV dataset read 8,198,511,150 score-and-position pairs.
7,651,541,764 of those pairs carry a zero score, and 2,225,454 of those zero
scores carry a position other than `-50`. `GRCh38:chrX:100627272:C:A` is one of
them and reports `"gain_score":"0.00","gain_position":-49`. The model reports the
position of its own extremum even when that extremum rounds to `0.00`. Read a
position only where the score beside it is non-zero. Never infer a zero score
from a position of `-50`, and never infer a position of `-50` from a zero score.
[`planning/artifacts/0040-score-value-determinism.md`](../planning/artifacts/0040-score-value-determinism.md)
records the scan.

`provenance.kind` names the route that produced a value. A consumer storing a
score stores that field with it.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-engine \
  --lib the_precomputed_and_model_routes_do_not_always_report_the_same_value >/dev/null
printf 'the precomputed and model routes do not always report the same value\n' | mustmatch like 'the precomputed and model routes do not always report the same value'
```

## What a deployment setting moves

`--model-workers` and `--model-threads` move no modeled score. They move no
position, no status and no rejection reason. Two deployments running the same
assets under different worker and thread settings return the same answer.
Measurement supports that. No gate proves it. The paragraph below the block
names what the measurement covered.

`--model-threads` does move the reported `effective_cpu_policy` and the
`scoring_identity` derived from it. `--model-workers` moves neither. The
effective policy renders `sequential:{threads}/1` and the worker count never
enters it.

The identity also carries the PangoPup version and the runtime profile. Between
two deployments of the same assets on the same PangoPup version, a
`scoring_identity` change therefore reports a thread-count change and no change
of answer. A PangoPup version change moves the identity too, and a version
change can move an answer.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-cli \
  --features service-test-fixtures --test http_service_lifecycle \
  a_deployment_worker_and_thread_setting_changes_no_modeled_score >/dev/null
printf 'a deployment worker or thread setting changes no modeled score\n' | mustmatch like 'a deployment worker or thread setting changes no modeled score'
```

That gate test runs a stand-in model with two operators, `MaxPool` and `Concat`.
Neither operator sums across threads. That model's arithmetic cannot reorder.
Only the production model can reorder a floating-point sum. No gate runs the
production model. The production proof is
[`planning/artifacts/0040-score-value-determinism.md`](../planning/artifacts/0040-score-value-determinism.md).
That artifact records four runs on one host against one build.
`retained_assets_score_identically_under_two_cpu_policies` repeats the
comparison on demand against retained production assets. Read the statement
above as a measured result on the shipped v0.5.0 assets. It does not prove the
same result for every host and build.

## What a consumer can cite

A consumer pinning a threshold reads two documents rather than a source line.
`README.md` states what a score value is beside the fields it describes.
`architecture/compatibility.md` states what a consumer must pin and record.

```bash
readme=$(cat ../README.md)
for statement in \
  'A score is an exact decimal in hundredths.' \
  '`gain_score` runs from `0.00` through `1.00` and `loss_score` runs from `0.00` through `-1.00`.' \
  'A rendered score always carries exactly two decimal places.' \
  'A threshold finer than one hundredth cannot be compared against a PangoPup score.' \
  '[What a score value is](spec/score-value.md)'; do
  printf '%s' "$readme" | rg -F -- "$statement" >/dev/null
done
score_values=$(awk '/^## Score values$/ { on=1; next } on && /^## / { exit } on' ../architecture/compatibility.md)
for statement in \
  'A score value is one of 101 exact hundredths.' \
  'The model rounds half to even.' \
  'The representable neighbours of 0.106 are `0.10` and `0.11`.' \
  'A precomputed score and a modeled score are not interchangeable.' \
  'Store `provenance.kind` beside every score a system retains.' \
  'A worker or thread setting changes no score.'; do
  printf '%s' "$score_values" | rg -F -- "$statement" >/dev/null
done
printf 'a consumer can cite a document for every statement above\n' | mustmatch like 'a consumer can cite a document for every statement above'
```
