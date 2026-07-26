# Ticket 021 — Complete-request CPU policy

Date: 2026-07-26

## Contract

This retained run compares exactly eight ONNX Runtime CPU policies through the
complete production `VariantScorer`, not raw context inference. Every process
authenticates and opens the same model, RefSeq sequence index, and GENCODE mask,
then checks every warmup and sample against the frozen M09 one-strand and M10
two-strand public records.

Normal tests do not run this measurement or open production assets.

## Coordinator commands

Retain `lscpu -e=CPU,CORE,SOCKET,ONLINE`, then build without running:

```text
cargo test --locked --release -p pangopup-engine --test cpu_policy_measurement --no-run
```

Run every policy in a fresh process with no concurrent benchmark work:

```text
PANGOPUP_CPU_POLICY='<POLICY>' taskset -c 0,2,4,6,8,10,12,14 \
  cargo test --locked --release -p pangopup-engine \
  --test cpu_policy_measurement \
  complete_variant_cpu_policy_release_measurement \
  -- --ignored --exact --nocapture
```

The exact policies are `sequential:auto/1`, `sequential:1/1`,
`sequential:2/1`, `sequential:4/1`, `sequential:8/1`, `parallel:1/2`,
`parallel:1/4`, and `parallel:1/8`. The test rejects another affinity or policy
spelling. Its one JSON object includes asset identities, component-open times,
runtime versions, high-water RSS, and exact M09/M10 records and timing.

## Host and raw results

The coordinator ran every candidate without concurrent benchmark work on the
same AMD Ryzen 7 5825U host used by the earlier model evidence. Topology was:

```text
$ lscpu -e=CPU,CORE,SOCKET,ONLINE
CPU CORE SOCKET ONLINE
  0    0      0    yes
  1    0      0    yes
  2    1      0    yes
  3    1      0    yes
  4    2      0    yes
  5    2      0    yes
  6    3      0    yes
  7    3      0    yes
  8    4      0    yes
  9    4      0    yes
 10    5      0    yes
 11    5      0    yes
 12    6      0    yes
 13    6      0    yes
 14    7      0    yes
 15    7      0    yes
```

Every process reported the required allow-list
`0,2,4,6,8,10,12,14`, Linux x86-64, `ort` 2.0.0-rc.12, ONNX Runtime
1.24.2, Pangopup 0.1.0, and these exact assets:

- model bundle
  `sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43`;
- reference bundle
  `sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f`;
- reference member
  `sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82`;
  and
- mask 6,703,320 bytes,
  `714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702`.

All candidates reproduced M09's one record and M10's two records exactly on
every warmup and sample.

| Policy | RSS KiB | M09 p50 / p95 ns | M10 p50 / p95 ns | M09 / M10 p50 ratio | Eligible |
|---|---:|---:|---:|---:|---|
| sequential `auto/1` | 125,904 | 2,266,311,419 / 6,268,143,052 | 8,796,295,868 / 14,148,966,809 | 0.4670 / 1.0148 | no |
| sequential `1/1` | 123,804 | 4,852,452,181 / 5,275,305,633 | 8,667,791,549 / 11,858,561,697 | 1.0000 / 1.0000 | baseline |
| sequential `2/1` | 123,944 | 2,857,313,961 / 3,065,677,863 | 4,719,429,993 / 5,441,475,437 | 0.5888 / 0.5445 | yes |
| sequential `4/1` | 123,536 | 1,300,881,873 / 1,356,353,741 | 2,735,143,228 / 2,792,573,111 | 0.2681 / 0.3156 | yes |
| sequential `8/1` | 124,800 | 1,049,910,580 / 1,258,726,620 | 2,644,959,697 / 2,920,696,761 | 0.2164 / 0.3051 | yes |
| parallel `1/2` | 123,656 | 5,617,724,252 / 5,959,647,488 | 8,701,602,192 / 10,268,584,363 | 1.1577 / 1.0039 | no |
| parallel `1/4` | 124,048 | 4,087,057,529 / 4,490,996,584 | 8,389,173,642 / 8,434,301,553 | 0.8423 / 0.9679 | no |
| parallel `1/8` | 124,316 | 4,395,735,002 / 4,548,089,855 | 9,196,707,928 / 10,761,203,916 | 0.9059 / 1.0610 | no |

Component open/initialization nanoseconds, in
`model/reference/mask/total` order:

```text
sequential:auto/1  1632742557 / 1027821361 / 14618611 / 2675183711
sequential:1/1     4056401908 / 1018115139 /  9757434 / 5084275924
sequential:2/1     3317361735 /  553879813 /  5079627 / 3876322557
sequential:4/1     1301163417 /  459167706 /  3985708 / 1764317793
sequential:8/1     1047833551 /  443908878 /  4026108 / 1495769458
parallel:1/2       2649975598 /  496828473 /  4545497 / 3151350561
parallel:1/4       2333301414 /  439658371 /  4080134 / 2777041231
parallel:1/8       2283692664 /  438312819 /  3972322 / 2725978966
```

The retained candidate JSON lines are:

```jsonl
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"sequential:auto/1","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":1632742557,"reference_ns":1027821361,"mask_ns":14618611,"total_ns":2675183711},"maximum_rss_kib":125904,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":2266311419,"p95_ns":6268143052},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":8796295868,"p95_ns":14148966809}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"sequential:1/1","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":4056401908,"reference_ns":1018115139,"mask_ns":9757434,"total_ns":5084275924},"maximum_rss_kib":123804,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":4852452181,"p95_ns":5275305633},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":8667791549,"p95_ns":11858561697}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"sequential:2/1","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":3317361735,"reference_ns":553879813,"mask_ns":5079627,"total_ns":3876322557},"maximum_rss_kib":123944,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":2857313961,"p95_ns":3065677863},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":4719429993,"p95_ns":5441475437}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"sequential:4/1","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":1301163417,"reference_ns":459167706,"mask_ns":3985708,"total_ns":1764317793},"maximum_rss_kib":123536,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":1300881873,"p95_ns":1356353741},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":2735143228,"p95_ns":2792573111}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"sequential:8/1","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":1047833551,"reference_ns":443908878,"mask_ns":4026108,"total_ns":1495769458},"maximum_rss_kib":124800,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":1049910580,"p95_ns":1258726620},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":2644959697,"p95_ns":2920696761}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"parallel:1/2","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":2649975598,"reference_ns":496828473,"mask_ns":4545497,"total_ns":3151350561},"maximum_rss_kib":123656,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":5617724252,"p95_ns":5959647488},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":8701602192,"p95_ns":10268584363}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"parallel:1/4","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":2333301414,"reference_ns":439658371,"mask_ns":4080134,"total_ns":2777041231},"maximum_rss_kib":124048,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":4087057529,"p95_ns":4490996584},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":8389173642,"p95_ns":8434301553}]}
{"schema":"pangopup-cpu-policy-measurement-v1","policy":"parallel:1/8","affinity":"0,2,4,6,8,10,12,14","target_os":"linux","target_arch":"x86_64","runtime":{"pangopup_model":"0.1.0","ort_crate":"2.0.0-rc.12","onnx_runtime":"1.24.2"},"assets":{"model_bundle":"sha256:4d8f2b8e7ee2dbf5d555c56693280d78d04ee2d0cf3346dfc35066e2a90aae43","reference_bundle":"sha256:7c28334e1829505863ff77dba78c4cbc0d8ebe655f68c30ad70ab4fdc36adc5f","reference_member":"sha256:cdec4b6230c3b660b658f71e11cb79d760d74f906873e81dc53ba7347ee3da82","mask_bytes":6703320,"mask_sha256":"714b1ac12dd6053a09841fe03c0ebb20fd027f6ef50732f03e7a10b7918dd702"},"component_open":{"model_ns":2283692664,"reference_ns":438312819,"mask_ns":3972322,"total_ns":2725978966},"maximum_rss_kib":124316,"cases":[{"id":"M09-insertion-short-plus","qualified":true,"records":1,"warmups":2,"samples":7,"p50_ns":4395735002,"p95_ns":4548089855},{"id":"M10-insertion-short-both","qualified":true,"records":2,"warmups":2,"samples":7,"p50_ns":9196707928,"p95_ns":10761203916}]}
```

The seven-sample p95 is the maximum observation and is not a selection gate.

## Mechanical selection

The baseline's twice-RSS ceiling was 247,608 KiB; every candidate stayed below
it. Sequential `2/1`, `4/1`, and `8/1` improved both p50 values by at least 20
percent. Sequential `8/1` had the lowest worse-case ratio, 0.3051, so it is the
**measured frontier winner for this host**.

Sequential `auto/1` did not qualify because M10's p50 ratio was 1.0148. The
**selected ordinary default therefore remains sequential `1/1`**. Pangopup
does not promote the host-specific fixed `8/1` count as a portable default.

## Selected-policy rerun and complete qualification

The fresh selected-default `1/1` rerun remained exact and reported RSS 123,388
KiB, M09 p50/p95 4,152,632,751 / 4,251,535,432 ns, and M10 p50/p95
8,540,380,137 / 9,045,722,717 ns. Component
model/reference/mask/total open values were 2,755,885,502 / 440,983,614 /
3,968,093 / 3,200,838,242 ns.

The coordinator then ran:

```text
taskset -c 0,2,4,6,8,10,12,14 \
  cargo test --locked --release -p pangopup-engine \
  --test production_qualification \
  retained_production_assets_match_all_masked_model_cases \
  -- --ignored --exact --nocapture
```

It reproduced all 14 frozen cases and 21 public records with the exact accepted
model, reference, mask, and post-ensemble receipt identities. Inspection of the
retained ONNX Runtime static archive with `nm` found no unresolved GOMP/OMP
symbols, so the `ort` warning about OpenMP builds does not explain the observed
intra-op behavior.

## Limits

These are measurements from one CPU host under one affinity. They are not a
portable latency guarantee, concurrent-service benchmark, HTTP benchmark, or
reason to publish a fixed thread count as the default.
