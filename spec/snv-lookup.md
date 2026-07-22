# Typed SNV lookup

This spec creates its own attributed production-path fixture and certified
bundle. It includes the real WRAP53/TP53 rows at `chr17:7686072 G>T`, two
source-reference exception shapes, a mixed locus, a proved miss, and more than
100 distinct ordinary hits.

```bash
../tests/fixtures/make-lookup-fixture.sh ../target/spec/snv-lookup-contract
pangopup-build build --source ../target/spec/snv-lookup-contract/source --reference ../target/spec/snv-lookup-contract/reference.fa.gz --output ../target/spec/snv-lookup-contract/bundle | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | rg -o '"status":"built","bundle_id":"sha256:<digest>"' | mustmatch like '"status":"built","bundle_id":"sha256:<digest>"'
pangopup-build verify ../target/spec/snv-lookup-contract/bundle | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"verified","bundle_id":"sha256:<digest>","members_verified":2}'
```

Default lookup returns both genes in order; a shared optional filter narrows
the same score without changing it. RefSeq accessions normalize to `chr` form.

```bash
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:17:7686072:G:T | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"assembly":"GRCh38","contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":[{"gene":"ENSG00000141499","gain_score":"0.35","gain_position":25,"loss_score":"0.00","loss_position":-50},{"gene":"ENSG00000141510","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:<digest>","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}'
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:NC_000017.11:7686072:G:T --gene ENSG00000141499 | rg -o '"contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":\[\{"gene":"ENSG00000141499","gain_score":"0.35"' | mustmatch like '"contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":[{"gene":"ENSG00000141499","gain_score":"0.35"'
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr17:7686072:G:T --gene ENSG00000141510 | rg -o '"records":\[\{"gene":"ENSG00000141510","gain_score":"0.00"' | mustmatch like '"records":[{"gene":"ENSG00000141510","gain_score":"0.00"'
```

Miss, source ambiguity, and simultaneous ordinary/ambiguity results are
distinct success statuses. A concrete reference mismatch is a miss.

```bash
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr1:1:A:C | rg -o '"status":"not_found","records":\[\],"source_reference_ambiguities":\[\]' | mustmatch like '"status":"not_found","records":[],"source_reference_ambiguities":[]'
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr1:105:A:C | rg -o '"status":"ambiguous_source_reference","records":\[\],"source_reference_ambiguities":\[\{"gene":"ENSG00000000004","source_ref":"N","published_alts":\["C","G","T"\],"omitted_alt":"A"\}\]' | mustmatch like '"status":"ambiguous_source_reference","records":[],"source_reference_ambiguities":[{"gene":"ENSG00000000004","source_ref":"N","published_alts":["C","G","T"],"omitted_alt":"A"}]'
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr1:3:G:C | rg -o '"status":"mixed","records":\[\{"gene":"ENSG00000000003"[^]]*\],"source_reference_ambiguities":\[\{"gene":"ENSG00000000004"' | rg -o '"status":"mixed"' | mustmatch like '"status":"mixed"'
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr17:7686072:A:T | rg -o '"status":"not_found"' | mustmatch like '"status":"not_found"'
```

The complete JSONL bytes for every status, including found multiplicity and
the final LF on every object, match the contract exactly.

```bash
actual=../target/spec/snv-lookup-contract/status-matrix.actual
normalized=../target/spec/snv-lookup-contract/status-matrix.normalized
expected=../target/spec/snv-lookup-contract/status-matrix.expected
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle \
  --variant GRCh38:chr17:7686072:G:T \
  --variant GRCh38:chr1:105:A:C \
  --variant GRCh38:chr1:3:G:C \
  --variant GRCh38:chr1:1:A:C > "$actual"
sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' "$actual" > "$normalized"
printf '%s\n' \
  '{"assembly":"GRCh38","contig":"chr17","position":7686072,"ref":"G","alt":"T","status":"found","records":[{"gene":"ENSG00000141499","gain_score":"0.35","gain_position":25,"loss_score":"0.00","loss_position":-50},{"gene":"ENSG00000141510","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:<digest>","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}' \
  '{"assembly":"GRCh38","contig":"chr1","position":105,"ref":"A","alt":"C","status":"ambiguous_source_reference","records":[],"source_reference_ambiguities":[{"gene":"ENSG00000000004","source_ref":"N","published_alts":["C","G","T"],"omitted_alt":"A"}],"provenance":{"kind":"precomputed","bundle_id":"sha256:<digest>","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}' \
  '{"assembly":"GRCh38","contig":"chr1","position":3,"ref":"G","alt":"C","status":"mixed","records":[{"gene":"ENSG00000000003","gain_score":"0.00","gain_position":-50,"loss_score":"0.00","loss_position":-50}],"source_reference_ambiguities":[{"gene":"ENSG00000000004","source_ref":"N","published_alts":["A","C","G"],"omitted_alt":"T"}],"provenance":{"kind":"precomputed","bundle_id":"sha256:<digest>","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}' \
  '{"assembly":"GRCh38","contig":"chr1","position":1,"ref":"A","alt":"C","status":"not_found","records":[],"source_reference_ambiguities":[],"provenance":{"kind":"precomputed","bundle_id":"sha256:<digest>","source_doi":"10.5281/zenodo.15649338","source_archive_md5":"679ef0b50e511b6102b4b88fbf811108","masked":true,"window":50}}' > "$expected"
cmp "$expected" "$normalized"
printf 'exact JSONL status matrix\n' | mustmatch like 'exact JSONL status matrix'
```

Table output is byte-stable, tab separated, and has one header. Batch output
preserves request order; 10 and 100 distinct requests produce exactly that many
JSON lines.

```bash
table_actual=../target/spec/snv-lookup-contract/status-matrix.table.actual
table_normalized=../target/spec/snv-lookup-contract/status-matrix.table.normalized
table_expected=../target/spec/snv-lookup-contract/status-matrix.table.expected
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle \
  --variant GRCh38:chr17:7686072:G:T \
  --variant GRCh38:chr1:105:A:C \
  --variant GRCh38:chr1:3:G:C \
  --variant GRCh38:chr1:1:A:C --format table > "$table_actual"
sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' "$table_actual" > "$table_normalized"
printf '%b\n' \
  'ASSEMBLY	CONTIG	POS	REF	ALT	STATUS	GENE	GAIN_SCORE	GAIN_POS	LOSS_SCORE	LOSS_POS	SOURCE_REF	PUBLISHED_ALTS	OMITTED_ALT	BUNDLE_ID' \
  'GRCh38	chr17	7686072	G	T	found	ENSG00000141499	0.35	25	0.00	-50	.	.	.	sha256:<digest>' \
  'GRCh38	chr17	7686072	G	T	found	ENSG00000141510	0.00	-50	0.00	-50	.	.	.	sha256:<digest>' \
  'GRCh38	chr1	105	A	C	ambiguous_source_reference	ENSG00000000004	.	.	.	.	N	C,G,T	A	sha256:<digest>' \
  'GRCh38	chr1	3	G	C	mixed	ENSG00000000003	0.00	-50	0.00	-50	.	.	.	sha256:<digest>' \
  'GRCh38	chr1	3	G	C	mixed	ENSG00000000004	.	.	.	.	N	A,C,G	T	sha256:<digest>' \
  'GRCh38	chr1	1	A	C	not_found	.	.	.	.	.	.	.	.	sha256:<digest>' > "$table_expected"
cmp "$table_expected" "$table_normalized"
test "$(wc -l < "$table_actual")" -eq 7
test "$(tail -c 1 "$table_actual" | od -An -tu1 | tr -d ' ')" = 10
variants=$(for pos in $(seq 5 14); do printf ' --variant GRCh38:chr1:%s:A:C' "$pos"; done); eval "pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle $variants" | wc -l | tr -d ' ' | mustmatch like '10'
variants=$(for pos in $(seq 5 104); do printf ' --variant GRCh38:chr1:%s:A:C' "$pos"; done); eval "pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle $variants" | wc -l | tr -d ' ' | mustmatch like '100'
```

Every primary spelling and all 25 exact RefSeq aliases are accepted and
normalized. The fixture's exact sequence ends are valid; the next position is
not.

```bash
bundle=../target/spec/snv-lookup-contract/bundle
for number in $(seq 1 22); do
  for spelling in "$number" "chr$number"; do
    pangopup lookup --bundle "$bundle" --variant "GRCh38:$spelling:1:A:C" | rg -q "\"contig\":\"chr$number\""
  done
done
for pair in X=chrX chrX=chrX Y=chrY chrY=chrY M=chrM chrM=chrM; do
  spelling=${pair%%=*}; normalized=${pair#*=}
  pangopup lookup --bundle "$bundle" --variant "GRCh38:$spelling:1:A:C" | rg -q "\"contig\":\"$normalized\""
done
for pair in \
  NC_000001.11=chr1 NC_000002.12=chr2 NC_000003.12=chr3 NC_000004.12=chr4 \
  NC_000005.10=chr5 NC_000006.12=chr6 NC_000007.14=chr7 NC_000008.11=chr8 \
  NC_000009.12=chr9 NC_000010.11=chr10 NC_000011.10=chr11 NC_000012.12=chr12 \
  NC_000013.11=chr13 NC_000014.9=chr14 NC_000015.10=chr15 NC_000016.10=chr16 \
  NC_000017.11=chr17 NC_000018.10=chr18 NC_000019.10=chr19 NC_000020.11=chr20 \
  NC_000021.9=chr21 NC_000022.11=chr22 NC_000023.11=chrX NC_000024.10=chrY \
  NC_012920.1=chrM; do
  spelling=${pair%%=*}; normalized=${pair#*=}
  pangopup lookup --bundle "$bundle" --variant "GRCh38:$spelling:1:A:C" | rg -q "\"contig\":\"$normalized\""
done
pangopup lookup --bundle "$bundle" --variant GRCh38:chr1:106:A:C >/dev/null
pangopup lookup --bundle "$bundle" --variant GRCh38:NC_000017.11:7686072:G:T >/dev/null
for invalid in chr01 chrx chrMT MT Chr1 NC_000001.1 ' chr1' 'chr1 '; do
  if output=$(pangopup lookup --bundle "$bundle" --variant "GRCh38:$invalid:1:A:C" 2>&1); then exit 1; else status=$?; fi
  test "$status" -eq 2
  printf '%s' "$output" | rg -q '"code":"INVALID_VARIANT"'
done
for invalid in GRCh38:chr1:0:A:C GRCh38:chr1:107:A:C GRCh38:chr2:2:A:C GRCh38:chr1:4294967296:A:C; do
  if output=$(pangopup lookup --bundle "$bundle" --variant "$invalid" 2>&1); then exit 1; else status=$?; fi
  test "$status" -eq 2
  printf '%s' "$output" | rg -q '"code":"INVALID_VARIANT"'
done
printf 'exact alias and bound matrix\n' | mustmatch like 'exact alias and bound matrix'
```

Both `REF=N` coordinates return the same gene ambiguity for all twelve
distinct concrete base pairs. The gene filter applies independently to the
ambiguity-only and mixed cases.

```bash
bundle=../target/spec/snv-lookup-contract/bundle
for position_shape in '105|{"gene":"ENSG00000000004","source_ref":"N","published_alts":["C","G","T"],"omitted_alt":"A"}' '3|{"gene":"ENSG00000000004","source_ref":"N","published_alts":["A","C","G"],"omitted_alt":"T"}'; do
  position=${position_shape%%|*}; shape=${position_shape#*|}
  for reference in A C G T; do
    for alternate in A C G T; do
      test "$reference" = "$alternate" && continue
      pangopup lookup --bundle "$bundle" --variant "GRCh38:chr1:$position:$reference:$alternate" | rg -Fq "$shape"
    done
  done
done
pangopup lookup --bundle "$bundle" --variant GRCh38:chr1:105:A:C --gene ENSG00000000004 | rg -Fq '"status":"ambiguous_source_reference","records":[],"source_reference_ambiguities":[{"gene":"ENSG00000000004"'
pangopup lookup --bundle "$bundle" --variant GRCh38:chr1:105:A:C --gene ENSG00000000003 | rg -Fq '"status":"not_found","records":[],"source_reference_ambiguities":[]'
pangopup lookup --bundle "$bundle" --variant GRCh38:chr1:3:G:C --gene ENSG00000000003 | rg -Fq '"status":"found","records":[{"gene":"ENSG00000000003"'
pangopup lookup --bundle "$bundle" --variant GRCh38:chr1:3:G:C --gene ENSG00000000004 | rg -Fq '"status":"ambiguous_source_reference","records":[],"source_reference_ambiguities":[{"gene":"ENSG00000000004"'
printf 'exact REF=N pair and filter matrix\n' | mustmatch like 'exact REF=N pair and filter matrix'
```

The grammar and error vocabulary are closed. These failures write one compact
JSON error to stderr and no result bytes.

```bash run id=invalid-variant exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh37:chr1:1:A:C
```

```text expect=invalid-variant contains
{"status":"error","code":"INVALID_VARIANT"
```

```bash run id=invalid-gene exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr1:1:A:C --gene TP53
```

```text expect=invalid-gene contains
{"status":"error","code":"INVALID_GENE"
```

```bash run id=bundle-io exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/does-not-exist --variant GRCh38:chr1:1:A:C
```

```text expect=bundle-io contains
{"status":"error","code":"BUNDLE_IO"
```

```bash run id=zero-padded exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr01:1:A:C
```

```text expect=zero-padded contains
{"status":"error","code":"INVALID_VARIANT"
```

Contig syntax is rejected before touching a missing bundle.

```bash run id=invalid-before-open exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/does-not-exist --variant GRCh38:chr01:1:A:C
```

```text expect=invalid-before-open contains
{"status":"error","code":"INVALID_VARIANT"
```

```bash run id=position-overflow exit=2 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/bundle --variant GRCh38:chr1:4294967296:A:C
```

```text expect=position-overflow contains
{"status":"error","code":"INVALID_VARIANT"
```

Compatibility is typed separately from structural invalidity. Cheap open does
not hash same-size members or traverse ordinary payload; addressed payload is
validated lazily and batch stdout remains transactional.

```bash
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/incompatible
perl -pi -e 's/pangopup\.bundle\.v1/pangopup.bundle.v2/' ../target/spec/snv-lookup-contract/incompatible/manifest.json
perl -0pi -e 's/\}\z/,"future_extension":true}/' ../target/spec/snv-lookup-contract/incompatible/manifest.json
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/incompatible-format
perl -pi -e 's/pangopup\.fixed11\.v1/pangopup.fixed11.v2/' ../target/spec/snv-lookup-contract/incompatible-format/manifest.json
perl -0pi -e 's/\}\z/,"future_extension":true}/' ../target/spec/snv-lookup-contract/incompatible-format/manifest.json
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/invalid-header
printf X | dd of=../target/spec/snv-lookup-contract/invalid-header/scores.pgi bs=1 seek=0 conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/touched-payload
payload=$(od -An -tu8 -j56 -N8 ../target/spec/snv-lookup-contract/touched-payload/scores.pgi | tr -d ' ')
printf '\200' | dd of=../target/spec/snv-lookup-contract/touched-payload/scores.pgi bs=1 seek=$((payload + 10)) conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/notice-substitution
printf X | dd of=../target/spec/snv-lookup-contract/notice-substitution/NOTICE bs=1 seek=0 conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/invalid-directory
directory=$(od -An -tu8 -j24 -N8 ../target/spec/snv-lookup-contract/invalid-directory/scores.pgi | tr -d ' ')
printf '\000' | dd of=../target/spec/snv-lookup-contract/invalid-directory/scores.pgi bs=1 seek="$directory" conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/invalid-tree
tree=$(od -An -tu8 -j40 -N8 ../target/spec/snv-lookup-contract/invalid-tree/scores.pgi | tr -d ' ')
printf '\000' | dd of=../target/spec/snv-lookup-contract/invalid-tree/scores.pgi bs=1 seek=$((tree + 28)) conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/invalid-exception
exception=$(od -An -tu8 -j72 -N8 ../target/spec/snv-lookup-contract/invalid-exception/scores.pgi | tr -d ' ')
printf '\001' | dd of=../target/spec/snv-lookup-contract/invalid-exception/scores.pgi bs=1 seek=$((exception + 1)) conv=notrunc status=none
cp -a ../target/spec/snv-lookup-contract/bundle ../target/spec/snv-lookup-contract/oversized-manifest
truncate -s 1048577 ../target/spec/snv-lookup-contract/oversized-manifest/manifest.json
pangopup lookup --bundle ../target/spec/snv-lookup-contract/notice-substitution --variant GRCh38:chr1:5:A:C | rg -o '"status":"found"' | mustmatch like '"status":"found"'
```

```bash run id=bundle-incompatible exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/incompatible --variant GRCh38:chr1:5:A:C
```

```text expect=bundle-incompatible contains
{"status":"error","code":"BUNDLE_INCOMPATIBLE","message":"incompatible bundle: bundle schema version"
```

```bash run id=index-format-incompatible exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/incompatible-format --variant GRCh38:chr1:5:A:C
```

```text expect=index-format-incompatible contains
{"status":"error","code":"BUNDLE_INCOMPATIBLE","message":"incompatible bundle: index format version"
```

```bash run id=bundle-invalid exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/invalid-header --variant GRCh38:chr1:5:A:C
```

```text expect=bundle-invalid contains
{"status":"error","code":"BUNDLE_INVALID"
```

```bash run id=directory-invalid exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/invalid-directory --variant GRCh38:chr1:5:A:C
```

```text expect=directory-invalid contains
{"status":"error","code":"BUNDLE_INVALID"
```

```bash run id=tree-invalid exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/invalid-tree --variant GRCh38:chr1:5:A:C
```

```text expect=tree-invalid contains
{"status":"error","code":"BUNDLE_INVALID"
```

```bash run id=exception-invalid exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/invalid-exception --variant GRCh38:chr1:5:A:C
```

```text expect=exception-invalid contains
{"status":"error","code":"BUNDLE_INVALID"
```

```bash run id=manifest-oversized exit=1 stream=stderr
pangopup lookup --bundle ../target/spec/snv-lookup-contract/oversized-manifest --variant GRCh38:chr1:5:A:C
```

```text expect=manifest-oversized contains
{"status":"error","code":"BUNDLE_INVALID"
```

```bash run id=lookup-corrupt exit=1 stream=stderr
result=../target/spec/snv-lookup-contract/transactional.stdout
pangopup lookup --bundle ../target/spec/snv-lookup-contract/touched-payload --variant GRCh38:chr1:5:A:C --variant GRCh38:chr1:3:G:C > "$result"
status=$?
test ! -s "$result"
exit "$status"
```

```text expect=lookup-corrupt contains
{"status":"error","code":"LOOKUP_CORRUPT"
```

Offline verification still catches the same-size NOTICE substitution that
cheap open deliberately permits.

```bash run id=notice-hash exit=1 stream=stderr
pangopup-build verify ../target/spec/snv-lookup-contract/notice-substitution
```

```text expect=notice-hash contains
{"status":"error","code":"BUNDLE_MEMBER_HASH"
```
