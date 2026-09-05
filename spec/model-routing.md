# Lookup-first model routing

The existing SNV bundle remains authoritative even when all explicitly
supplied fallback paths do not exist. No fallback component is opened on this
path.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  --reference-bundle /missing/reference \
  --mask /missing/mask \
  --model-bundle /missing/model \
  | rg -o '"kind":"precomputed"' | mustmatch like '"kind":"precomputed"'
```

`--model-only` explicitly bypasses SNV lookup. A complete explicit model tuple
is self-sufficient: no SNV bundle or active installation is required. The
default command still returns a genuinely covered Zenodo-derived row, while
the independent model-only command traverses the existing checked model route.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  | rg -o '"position":6801301.*"kind":"precomputed"' \
  | mustmatch like '"position":6801301,"ref":"G","alt":"A","status":"found","records":[{"gene":"ENSG00000010610","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed"'
pangopup lookup \
  --model-only \
  --variant GRCh38:chr1:5051:A:C \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"position":5051,"ref":"A","alt":"C","status":"found".*"kind":"model"' \
  | mustmatch like '"position":5051,"ref":"A","alt":"C","status":"found","records":[{"gene":"ENSG00000000001.1","gain_score":"0.33","gain_position":-50,"loss_score":"0.00","loss_position":-50,"warnings":["no_annotated_sites"]}],"source_reference_ambiguities":[],"provenance":{"kind":"model"'
pangopup lookup \
  --model-only \
  --variant GRCh38:chr1:5051:A:C \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  --format table \
  | tail -1 \
  | mustmatch like 'GRCh38	chr1	5051	A	C	found	ENSG00000000001.1	0.33	-50	0.00	-50	.	.	.	sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca'
```

Contradictory or duplicated model-only controls are usage errors.

```bash run id=model-only-bundle-conflict exit=2 stream=stderr
pangopup lookup --model-only --bundle /missing/snv --variant GRCh38:chr1:5051:A:C
```

```text expect=model-only-bundle-conflict contains
{"status":"error","code":"CLI_USAGE"
```

Request-only model rejections occur before model runtime admission. The engine owns these checks and scoring uses the same boundary. The command retains the existing rejection code, message, and exit status even when every supplied runtime path is missing.

```bash run id=model-only-early-request-rejection exit=2 stream=stderr
pangopup lookup --model-only --variant GRCh38:chr1:5051:A:TC --reference-bundle /missing/reference --mask /missing/mask --model-bundle /missing/model
```

```text expect=model-only-early-request-rejection contains
{"status":"error","code":"MODEL_REJECTED","message":"unsupported variant shape","details":null}
```

Lookup-first operation may open the SNV bundle to decide whether a variant needs the model. It rejects every model-required variant before it admits model-side assets. The first request-only rejection in input order ends the batch without partial output.

```bash run id=lookup-first-early-request-rejection exit=2 stream=stderr
pangopup lookup --bundle ../tests/fixtures/snv-regression/bundle --variant GRCh38:chr12:6801301:G:A --variant GRCh38:chr1:5051:A:TC --reference-bundle /missing/reference --mask /missing/mask --model-bundle /missing/model
```

```text expect=lookup-first-early-request-rejection contains
{"status":"error","code":"MODEL_REJECTED","message":"unsupported variant shape","details":null}
```

```bash run id=model-only-duplicate exit=2 stream=stderr
pangopup lookup --model-only --model-only --variant GRCh38:chr1:5051:A:C
```

```text expect=model-only-duplicate contains
{"status":"error","code":"CLI_USAGE"
```

A non-SNV requires the complete explicit fallback set.

```bash run id=model-assets-required exit=2 stream=stderr
pangopup lookup --bundle ../tests/fixtures/snv-regression/bundle --variant GRCh38:chr1:5051:A:AC
```

```text expect=model-assets-required contains
{"status":"error","code":"MODEL_ASSETS_REQUIRED"
```

Without fallback flags, a pure SNV miss keeps the legacy precomputed
`not_found` result.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr10:1:A:C \
  | mustmatch like '{"assembly":"GRCh38","contig":"chr10","position":1,"ref":"A","alt":"C","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:73a36d75c32db2bfdbe1b3098dd397e1bbe8575c64614136648d5bd8c49f0c60","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}'
```

The checked synthetic route traverses the real PGRREF01 reader, domains mmap,
ONNX Runtime CPU kernel, and compatible variant scorer. Exact model provenance
and the no-annotated-sites warning are present.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"status":"found".*"gene":"ENSG00000000001.1".*"warnings":\["no_annotated_sites"\].*"kind":"model".*"scoring_semantics":"pangopup-variant-score-v1".*"reference_profile":"pangopup-reference-route-test-v1".*"mask_bytes":260.*"window":50' \
  | mustmatch like '"status":"found","records":[{"gene":"ENSG00000000001.1","gain_score":"0.33","gain_position":-50,"loss_score":"0.00","loss_position":-50,"warnings":["no_annotated_sites"]}],"source_reference_ambiguities":[],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","model_bundle_id":"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca","model_profile":"pangopup-model-kernel-mini-v1","effective_cpu_policy":"sequential:1/1","reference_bundle_id":"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493","reference_profile":"pangopup-reference-route-test-v1","reference_sequence_set_sha256":"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3","mask_bytes":260,"mask_sha256":"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827","masked":true,"window":50'
```

Successful model results persist across CLI processes and retain byte-identical
public output.

```bash
cache="$XDG_CACHE_HOME/pangopup/model-results.sqlite3"
rm -f "$cache" "$cache-wal" "$cache-shm"
first=../target/spec/model-cache-first.jsonl
second=../target/spec/model-cache-second.jsonl
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  >"$first"
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  >"$second"
cmp "$first" "$second"
test -s "$cache"
printf 'persistent exact model cache\n' | mustmatch like 'persistent exact model cache'
```

Cache configuration without the complete fallback tuple is a usage error.

```bash run id=cache-needs-fallback exit=2 stream=stderr
pangopup lookup --bundle ../tests/fixtures/snv-regression/bundle --variant GRCh38:chr12:6801301:G:A --model-cache /tmp/pangopup.sqlite3
```

```text expect=cache-needs-fallback contains
{"status":"error","code":"CLI_USAGE"
```

Supplying the complete fallback tuple also routes a pure SNV lookup miss
through the model.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:C \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"position":5051,"ref":"A","alt":"C","status":"found".*"kind":"model"' \
  | mustmatch like '"position":5051,"ref":"A","alt":"C","status":"found","records":[{"gene":"ENSG00000000001.1","gain_score":"0.33","gain_position":-50,"loss_score":"0.00","loss_position":-50,"warnings":["no_annotated_sites"]}],"source_reference_ambiguities":[],"provenance":{"kind":"model"'
```

A mixed batch preserves request order and opens one shared fallback. A gene filter accepts the stable Ensembl form and the versioned GENCODE forms emitted by model results. Every accepted form filters on the stable identity after complete model scoring and can produce a modeled miss. The first command below submits the miniature model's reported gene unchanged and receives the same bytes.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --gene ENSG00000000001.1 \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -F -o '"status":"found","records":[{"gene":"ENSG00000000001.1"' \
  | mustmatch like '"status":"found","records":[{"gene":"ENSG00000000001.1"'
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"kind":"(precomputed|model)"' \
  | paste -sd, - \
  | mustmatch like '"kind":"precomputed","kind":"model"'
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:NC_000001.11:5051:A:AC \
  --gene ENSG00000000002 \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"contig":"chr1".*"status":"not_found","records":\[\].*"kind":"model"' \
  | mustmatch like '"contig":"chr1","position":5051,"ref":"A","alt":"AC","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"model"'
```

The adapter tests also accept the versioned `_PAR_Y` form and reject zero, leading-zero, missing, overflowing, repeated, and unknown suffixes. Rejected CLI filters retain exit 2 and `INVALID_GENE`.

The compact table keeps its original columns and identifies the model bundle.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  --format table \
  | tail -1 \
  | mustmatch like 'GRCh38	chr1	5051	A	AC	found	ENSG00000000001.1	0.33	-50	0.00	-50	.	.	.	sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca'
```

Partial fallback grammar, expected model rejection, and component-open failure
have stable exit classes and one redacted stderr line.

```bash run id=partial-fallback exit=2 stream=stderr
pangopup lookup --bundle ../tests/fixtures/snv-regression/bundle --variant GRCh38:chr1:5051:A:AC --mask /secret/mask
```

```text expect=partial-fallback contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=model-rejected exit=2 stream=stderr
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:TC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle
```

```text expect=model-rejected contains
{"status":"error","code":"MODEL_REJECTED"
```

```bash run id=model-reference-invalid exit=1 stream=stderr
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle /secret/reference-does-not-exist \
  --mask /secret/mask-must-not-open \
  --model-bundle /secret/model-must-not-open
```

```text expect=model-reference-invalid exact
{"status":"error","code":"REFERENCE_BUNDLE_INVALID","message":"reference bundle is invalid","details":null}
```

A late rejection leaves no bytes from an earlier authoritative request on
stdout.

```bash
output=../target/spec/model-routing-transactional.stdout
rm -f "$output"
if pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr1:5051:A:TC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  >"$output" 2>/dev/null; then exit 1; else status=$?; fi
test "$status" -eq 2
test ! -s "$output"
printf 'transactional model batch\n' | mustmatch like 'transactional model batch'
```
