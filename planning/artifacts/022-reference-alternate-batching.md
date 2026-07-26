# Ticket 022 — Reference/alternate batching evidence

Date: 2026-07-26

## Result

This first run is retained as historical evidence but is ineligible for final
selection. Code review found that the v2 candidate manifests declared dynamic
batch/length axes that were applied only to final graph metadata and not passed
to PyTorch's exporter. Corrected candidates require fresh raw, public, and
performance qualification in a new scratch root.

Under the first-run arithmetic, singleton would be retained with ordinary
sequential `1/1` runtime.
Both formal policy comparisons were inconclusive because at least one
singleton case varied by more than 20 percent across the three fresh-process
rounds. Even if the drift rule is ignored, neither candidate meets the
replacement gates: neither improves both M09 and M10 by 20 percent, and each
has material regressions.

## Corrected final experiment

Corrected v2 conversion changed both candidate graph and bundle identities.
All three corrected-matrix representations passed the independent raw oracle:
36 sequence evaluations, 432 channel arrays, and 45,756 scalar comparisons
with maximum absolute error `5.364418029785156e-7`. All also passed the 14
public cases and 21 ordered records exactly.

| Representation | Bundle identity | Graph SHA-256 | Model bytes | Bundle bytes |
| --- | --- | --- | ---: | ---: |
| accepted singleton | `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43` | `3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b` | 33,867,142 | 33,871,613 |
| corrected zero-padded | `sha256:bb5767d81c8b7e297e1b6212dd7e8a570e7af0d6478f73f7738dea59f327fefb` | `ec9f87aaa5597a6dbb515e1e215f2987157fd7f83cc4cec70b45fe5730c7a942` | 33,867,144 | 33,871,674 |
| corrected paired-strand | `sha256:4957ced0dc97a0aae74a07a4263796d93b9a5b6506c932ef9fec2cca799482dc` | `73c5662bbfda38166bea76da29b9eeb43d086057c4ab31046b1a5c32c6e20208` | 34,372,017 | 34,376,710 |

[`022-reference-alternate-batching-corrected-raw.jsonl`](022-reference-alternate-batching-corrected-raw.jsonl)
preserves all 19 corrected records verbatim; SHA-256 is
`7f60128ea6cb4d7857579c41b4d6d0b4f8fbc04fb8723edb53685ff40891962e`.

Corrected aggregate p50 is the median of the three fresh-process p50 values,
in seconds:

| Policy | Representation | M07 | M08 | M09 | M10 | M12 | M13 | Max RSS KiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `1/1` | singleton | 4.437 | 8.829 | 4.207 | 8.425 | 4.142 | 9.242 | 133,340 |
| `1/1` | zero-padded | 4.218 | 9.666 | 4.391 | 8.616 | 5.392 | 9.764 | 172,692 |
| `1/1` | paired | 4.280 | 8.207 | 4.173 | 8.326 | 4.054 | 8.201 | 138,340 |
| `8/1` | singleton | 1.310 | 2.724 | 1.441 | 2.946 | 1.810 | 3.210 | 134,184 |
| `8/1` | zero-padded | 2.611 | 3.344 | 3.336 | 3.921 | 2.776 | 3.271 | 179,288 |
| `8/1` | paired | 1.384 | 4.731 | 1.298 | 4.877 | 1.345 | 4.764 | 143,596 |

At `1/1`, singleton drift exceeded 20 percent for M07, M08, and M12.
At `8/1`, it exceeded 20 percent for M09, M10, M12, and M13. Both policy
comparisons are therefore formally inconclusive and retain singleton.

The independent gate calculation agrees:

- at `1/1`, zero-padded regressed M08 to `1.095x`, M12 to `1.302x`,
  and M13 to `1.056x`; paired stayed within the 5-percent regression gate but
  improved M09 only to `0.992x` and M10 only to `0.988x`, not the required
  `0.8x`;
- at `8/1`, zero-padded regressed five cases materially, including M09 to
  `2.315x`; paired improved M09 only to `0.901x` while regressing M07, M08,
  M10, and M13, including M10 to `1.655x`; and
- both candidates stayed below twice singleton RSS, and paired remained within
  the 5-percent model-size limit, so those gates do not change the result.

The ineligible corrected paired `parallel:1/8` diagnostic remained slow:
M09 p50 was 3.959 seconds and M10 was 8.695 seconds.

The mechanical final selection is the accepted singleton graph with ordinary
sequential `1/1`. Ordinary model/scorer construction is singleton-only.
Maintainer converter modes, closed v2 execution, the ignored harness, and both
tiny checked candidate fixtures remain under the approved reproducibility and
normal-test boundary.

The experiment changed no production asset. The selected asset remains the
Ticket 018 bundle
`sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`
with accepted graph SHA-256
`3c2760472ce0af5feb693f562716b6cdc6887a7d0a00b7b5ec8ddad2a2d31f6b`
as recorded by Ticket 018. The scratch singleton was an exact
graph-byte reconstruction with a different bundle manifest identity.

## Accuracy and identities

All three representations passed the independent raw oracle: 36 sequence
evaluations, 432 channel arrays, and 45,756 scalar comparisons, with maximum
absolute error `5.364418029785156e-7` against the `1e-5` limit. Each also
passed all 14 public cases and 21 ordered records exactly.

| Representation | Scratch bundle identity | Model bytes | Bundle bytes |
| --- | --- | ---: | ---: |
| singleton | `sha256:2fc4b436294adc274f4cd6e0c2d384c15ed30cbe743f276eb95c282a5e9c48d1` | 33,867,142 | 33,871,613 |
| zero-padded batch | `sha256:f1d0d994ad99a68d8bc50625596ad0bc70092eae82c43e1eae293b7c2aa395ee` | 33,867,144 | 33,871,674 |
| paired-strand batch | `sha256:b4efdb46b85d2ec18b1a9d10527b34b8f7ec61d19f947041b2975c0e99fff855` | 34,372,017 | 34,376,710 |

The paired graph is 1.49 percent larger than singleton, inside the 5-percent
limit. Both candidates stayed inside the two-times-RSS limit:

| Policy | Singleton max RSS KiB | Zero-padded max RSS KiB | Paired max RSS KiB |
| --- | ---: | ---: | ---: |
| sequential `1/1` | 133,236 | 172,780 | 138,520 |
| sequential `8/1` | 134,540 | 179,396 | 143,708 |

## Retained raw measurements

[`022-reference-alternate-batching-raw.jsonl`](022-reference-alternate-batching-raw.jsonl)
contains the 19 successful machine-readable records verbatim: three rotated
fresh-process rounds for each representation at sequential `1/1`, three at
sequential `8/1`, and one ineligible paired parallel `1/8` diagnostic. Its
SHA-256 is
`a1d536e75782b2d9dfb136527a9a72a6108df9dce43d40949b87b9909f349760`.
Every record binds the affinity, runtime, component identities and sizes,
open costs, peak RSS, invocation/logical-work accounting, padding, warmups,
samples, p50, and descriptive p95.

Aggregate p50 below is the median of the three per-process p50 values, in
seconds:

| Policy | Representation | M07 | M08 | M09 | M10 | M12 | M13 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `1/1` | singleton | 4.141 | 8.259 | 4.202 | 9.017 | 4.547 | 10.241 |
| `1/1` | zero-padded | 4.613 | 9.108 | 4.512 | 8.902 | 4.416 | 8.927 |
| `1/1` | paired | 4.082 | 8.966 | 4.538 | 9.342 | 4.317 | 8.706 |
| `8/1` | singleton | 1.284 | 2.609 | 1.171 | 4.913 | 1.906 | 3.392 |
| `8/1` | zero-padded | 2.778 | 3.453 | 2.738 | 2.788 | 2.379 | 2.756 |
| `8/1` | paired | 1.204 | 4.765 | 1.069 | 4.635 | 1.205 | 4.737 |

## Mechanical selection

At `1/1`, singleton drift exceeded 20 percent for M07 and M12, so the policy
comparison is formally inconclusive. The zero-padded M09 ratio was `1.074`
and paired M09 was `1.080`; neither is an improvement, much less the required
20 percent. Both also exceeded the 5-percent regression ceiling on M07, M08,
or M09.

At `8/1`, singleton drift exceeded 20 percent for M07, M09, M10, M12, and
M13, so this comparison is also inconclusive. Zero-padded regressed M09 to
`2.339x` singleton. Paired improved M09 only to `0.914x` and M10 to `0.943x`,
short of the required `0.8x`, while regressing M08 to `1.826x` and M13 to
`1.397x`.

The paired-only parallel `1/8` diagnostic was ineligible and slow: M09 p50 was
4.531 seconds and M10 p50 was 11.415 seconds.

The first attempted diagnostic used the invalid harness label
`PANGOPUP_MEASUREMENT_ROUND=diagnostic` and failed before opening assets. It
was rerun successfully with round `1`; only the successful JSON appears in the
retained raw file.

For this now-ineligible first run, singleton wins by the ticket's explicit retain-on-drift and
minimum-improvement rules. Candidate converter modes and the ignored
experimental harness remain for reproducibility. Ordinary runtime opening and
scorer construction accept singleton only; both tiny candidate fixtures remain
to exercise retained experimental execution, while losing ordinary dispatch
was removed.

## First-run selected proof

The coordinator reran the accepted Ticket 018 singleton through ordinary
model/scorer construction in two fresh pinned processes:

| Policy | RSS KiB | M07 p50 ns | M08 p50 ns | M09 p50 ns | M10 p50 ns | M12 p50 ns | M13 p50 ns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| sequential `1/1` | 133,064 | 4,303,030,821 | 10,990,903,581 | 4,997,240,082 | 8,730,702,473 | 4,215,945,983 | 8,271,444,155 |
| sequential `8/1` | 134,096 | 1,228,605,201 | 2,641,022,598 | 1,395,728,073 | 2,221,837,410 | 1,030,891,987 | 2,280,794,628 |

The two successful machine-readable records are preserved verbatim in
[`022-reference-alternate-batching-selected-rerun.jsonl`](022-reference-alternate-batching-selected-rerun.jsonl),
SHA-256
`72f54be3087bb9380c1411db2e4f0443a4d22705d29fa5471d25185cb9cd1a37`.

The selected raw qualification then passed the exact accepted model identity:
14 cases, 18 strand paths, 36 sequence evaluations, 432 channel arrays, 45,756
scalar comparisons, and maximum absolute error
`5.364418029785156e-7`. The selected public qualification passed all 14 cases
and 21 records with accepted reference identity
`sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
mask SHA-256
`714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`,
and post-ensemble receipt SHA-256
`3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b`.
No asset changed.

## Corrected candidate rebuild handoff

Code review remediation passes dynamic batch/length axes into PyTorch export
for both v2 representations while preserving the exact historical singleton
conversion. The tiny generator already emitted symbolic axes directly, so its
regenerated graph bytes did not change:

| Candidate | Mini graph bytes | Mini graph SHA-256 | Mini bundle identity |
| --- | ---: | --- | --- |
| zero-padded | 319 | `8b0c5e88dba199edd7b8f9e4a0255a3bff6bbe063299af7fb558206564b10194` | `sha256:398f5cf1a16bc727080b66d3d9c374834b73e72a13fb1101b9f34a510da8e621` |
| paired-strand | 591 | `c7aa66c35de5897b4f126053d920535d32f7cee6081207b9e9c74aef5f9f7ee2` | `sha256:2c8ddc93b3db9e07c07806b3dd853fd57beef3f4fe97719338f8e5f099068e76` |

After the same design reviewer approves the revised retained-fixture boundary,
the coordinator creates a new root rather than modifying
`pangopup-model-022`:

```text
test ! -e /home/ian/workspace/data/pangopup-model-022-corrected
mkdir --mode=700 /home/ian/workspace/data/pangopup-model-022-corrected

cargo run --locked --release -p pangopup-build --bin pangopup-build -- model convert \
  --upstream /home/ian/foss/Pangolin \
  --python /home/ian/workspace/repos/pangopup/tools/pangolin-model/.venv/bin/python \
  --evidence /home/ian/workspace/repos/pangopup/tests/fixtures/pangolin-model-v1 \
  --output /home/ian/workspace/data/pangopup-model-022-corrected/zero-padded \
  --representation zero-padded-batch

cargo run --locked --release -p pangopup-build --bin pangopup-build -- model convert \
  --upstream /home/ian/foss/Pangolin \
  --python /home/ian/workspace/repos/pangopup/tools/pangolin-model/.venv/bin/python \
  --evidence /home/ian/workspace/repos/pangopup/tests/fixtures/pangolin-model-v1 \
  --output /home/ian/workspace/data/pangopup-model-022-corrected/paired-strand \
  --representation paired-strand-batch
```

The coordinator records corrected model/bundle identities and compares them
with the ineligible first-run identities. If bytes differ, both corrected
candidates repeat the complete raw/public qualification and the full rotated
performance matrix against the unchanged accepted Ticket 018 singleton.

Corrected construction produced new bundle identities, so the full rerun is
required:

| Candidate | Ineligible first-run bundle | Corrected bundle | Corrected model bytes |
| --- | --- | --- | ---: |
| zero-padded | `sha256:f1d0d994ad99a68d8bc50625596ad0bc70092eae82c43e1eae293b7c2aa395ee` | `sha256:bb5767d81c8b7e297e1b6212dd7e8a570e7af0d6478f73f7738dea59f327fefb` | 33,867,144 |
| paired-strand | `sha256:b4efdb46b85d2ec18b1a9d10527b34b8f7ec61d19f947041b2975c0e99fff855` | `sha256:4957ced0dc97a0aae74a07a4263796d93b9a5b6506c932ef9fec2cca799482dc` | 34,372,017 |

The model byte counts remained unchanged, while both exact graph hashes and
bundle identities changed. The corrected qualifications and matrix above
supersede this construction handoff.

## Corrected selected proof

After the corrected matrix mechanically retained singleton, the coordinator
again reran the accepted Ticket 018 bundle through ordinary model/scorer
construction in two fresh pinned processes:

| Policy | RSS KiB | M07 p50 ns | M08 p50 ns | M09 p50 ns | M10 p50 ns | M12 p50 ns | M13 p50 ns |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| sequential `1/1` | 132,852 | 4,308,186,891 | 11,794,072,214 | 5,151,006,648 | 8,433,392,551 | 4,296,953,128 | 8,614,506,453 |
| sequential `8/1` | 134,044 | 1,215,176,059 | 3,557,446,318 | 2,790,069,880 | 3,540,379,038 | 1,669,394,743 | 3,084,015,168 |

The two successful records are preserved verbatim in
[`022-reference-alternate-batching-corrected-selected.jsonl`](022-reference-alternate-batching-corrected-selected.jsonl),
SHA-256
`730accf826431b6beb77c94d45c5d67e6f07bbde6f860e025e845dcea9a1a8e8`.

The final selected raw qualification passed the accepted bundle identity with
14 cases, 18 strand paths, 36 sequence evaluations, 432 channel arrays, 45,756
scalar comparisons, and maximum absolute error
`5.364418029785156e-7`. Final public qualification passed all 14 cases and 21
records with accepted reference identity
`sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`,
mask SHA-256
`714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`,
and post-ensemble receipt SHA-256
`3ac237ec676de1530a4cdebbb19d71a16d5e0a2a718788a0a0245891c2ad7d9b`.
No asset changed.
