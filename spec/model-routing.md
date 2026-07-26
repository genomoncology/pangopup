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
  | mustmatch like '{"assembly":"GRCh38","contig":"chr10","position":1,"ref":"A","alt":"C","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:fbb637198f52a28f93c43bf6803cfe7cfcb2d13351b518025ef78a65373610b5","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}'
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
  | mustmatch like '"status":"found","records":[{"gene":"ENSG00000000001.1","gain_score":"0.33","gain_position":-50,"loss_score":"0.00","loss_position":-50,"warnings":["no_annotated_sites"]}],"source_reference_ambiguities":[],"provenance":{"kind":"model","scoring_semantics":"pangopup-variant-score-v1","model_bundle_id":"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca","model_profile":"pangopup-model-kernel-mini-v1","reference_bundle_id":"sha256:6773713ad79462b8bfb2bce7f194041e85a0804b38f68282c965adc5f43f9493","reference_profile":"pangopup-reference-route-test-v1","reference_sequence_set_sha256":"sha256:afb720dad5979f65694dab6ae80a497ef56db434d7d346e79cdcb0e7da97e0b3","mask_bytes":260,"mask_sha256":"sha256:004f9f95be50b92fd5c67ca44a785e950c20e5455a903ad9350b68c91566f827","masked":true,"window":50'
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

A mixed batch preserves request order and opens one shared fallback. A stable
gene filter is applied after complete model scoring and can produce a modeled
miss.

```bash
pangopup lookup \
  --bundle ../tests/fixtures/snv-regression/bundle \
  --variant GRCh38:chr12:6801301:G:A \
  --variant GRCh38:chr1:5051:A:AC \
  --reference-bundle ../tests/fixtures/reference-route-test/bundle \
  --mask ../tests/fixtures/route-mask/domains.pgm \
  --model-bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle \
  | rg -o '"kind":"(precomputed|model)"' \
  | paste -sd, \
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
