# Full bundle build and certification

The production builder accepts explicit read-only source and reference paths,
publishes exactly one immutable three-file bundle, and emits one JSON line.
This fixture is deliberately synthetic and is not GRCh38 evidence.

```bash
chmod -R u+w ../target/spec/full-bundle 2>/dev/null || true
rm -rf ../target/spec/full-bundle
mkdir -p ../target/spec/full-bundle/source
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000001.tsv > ../target/spec/full-bundle/source/ENSG00000000001.tsv.gz
gzip -n -c ../tests/fixtures/full-build-source/ENSG00000000002.tsv > ../target/spec/full-bundle/source/ENSG00000000002.tsv.gz
cp ../tests/fixtures/full-build-reference.fa ../target/spec/full-bundle/reference.fa
chmod a-w ../target/spec/full-bundle/source/*.tsv.gz ../target/spec/full-bundle/reference.fa
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/reference.fa --output ../target/spec/full-bundle/plain | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"built","bundle_id":"sha256:<digest>","genes":2,"source_rows":15,"gene_loci":5,"ascending_members":1,"descending_members":1,"source_segments":2,"index_segments":3,"gap_transitions":0,"omitted_bases":0,"n_ref_loci":1,"n_omit_a":1,"n_omit_t":0}'
```

Full verification checks both non-manifest member hashes and every index
section, then reports the canonical manifest hash as bundle identity.

```bash
pangopup-build verify ../target/spec/full-bundle/plain | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"verified","bundle_id":"sha256:<digest>","members_verified":2}'
find ../target/spec/full-bundle/plain -mindepth 1 -maxdepth 1 -type f -printf '%f\n' | sort | mustmatch like "NOTICE
manifest.json
scores.pgi"
```

An ordinary single-member gzip FASTA is accepted. Rebuilding the identical
plain input at a second destination is byte deterministic, while rebuilding an
already published destination verifies and reuses it without mutation.

```bash
gzip -n -c ../target/spec/full-bundle/reference.fa > ../target/spec/full-bundle/reference.fa.gz
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/reference.fa.gz --output ../target/spec/full-bundle/gzip | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"built","bundle_id":"sha256:<digest>","genes":2}'
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/reference.fa --output ../target/spec/full-bundle/repeat | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"built","bundle_id":"sha256:<digest>","genes":2}'
cmp ../target/spec/full-bundle/plain/NOTICE ../target/spec/full-bundle/repeat/NOTICE
cmp ../target/spec/full-bundle/plain/scores.pgi ../target/spec/full-bundle/repeat/scores.pgi
cmp ../target/spec/full-bundle/plain/manifest.json ../target/spec/full-bundle/repeat/manifest.json
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/reference.fa --output ../target/spec/full-bundle/plain | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"already_present","bundle_id":"sha256:<digest>","genes":2}'
```

A reference mismatch fails before publication with deterministic bounded
details, leaves no staging scratch, and writes no stdout.

```bash run id=reference-mismatch exit=1 stream=stderr
sed '0,/ACGT/s//TCGT/' ../target/spec/full-bundle/reference.fa > ../target/spec/full-bundle/mismatch.fa
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/mismatch.fa --output ../target/spec/full-bundle/mismatch-bundle
```

```text expect=reference-mismatch like
{"status":"error","code":"REFERENCE_MISMATCH","message":"1 ordinary source references disagree with GRCh38.p14","details":{"examples":[{"contig":"chr1","expected":"A","gene":"ENSG00000000001","observed":"T","pos":1}],"mismatch_count":1}}
```

Missing, duplicate, and invalid required FASTA data have stable typed errors.

```bash run id=missing-accession exit=1 stream=stderr
sed '/>NC_012920.1/,$d' ../target/spec/full-bundle/reference.fa > ../target/spec/full-bundle/missing.fa
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/missing.fa --output ../target/spec/full-bundle/missing-bundle
```

```text expect=missing-accession contains
{"status":"error","code":"REFERENCE_MISSING_ACCESSION"
```

```bash run id=duplicate-accession exit=1 stream=stderr
cp ../target/spec/full-bundle/reference.fa ../target/spec/full-bundle/duplicate.fa
chmod u+w ../target/spec/full-bundle/duplicate.fa
printf '>NC_000001.11 duplicate\nACGT\n' >> ../target/spec/full-bundle/duplicate.fa
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/duplicate.fa --output ../target/spec/full-bundle/duplicate-bundle
```

```text expect=duplicate-accession contains
{"status":"error","code":"REFERENCE_DUPLICATE_ACCESSION"
```

```bash run id=invalid-sequence exit=1 stream=stderr
sed '0,/ACGT/s//ACG!/' ../target/spec/full-bundle/reference.fa > ../target/spec/full-bundle/invalid.fa
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/invalid.fa --output ../target/spec/full-bundle/invalid-bundle
```

```text expect=invalid-sequence contains
{"status":"error","code":"REFERENCE_INVALID_SEQUENCE"
```

An invalid existing destination is never replaced, and incomplete command
usage is also one typed JSON error line.

```bash run id=immutable-destination exit=1 stream=stderr
mkdir ../target/spec/full-bundle/occupied
printf x > ../target/spec/full-bundle/occupied/unrelated
pangopup-build build --source ../target/spec/full-bundle/source --reference ../target/spec/full-bundle/reference.fa --output ../target/spec/full-bundle/occupied
```

```text expect=immutable-destination contains
{"status":"error","code":"PUBLICATION_DESTINATION"
```

```bash run id=build-usage exit=2 stream=stderr
pangopup-build build --source ../target/spec/full-bundle/source
```

```text expect=build-usage like
{"status":"error","code":"CLI_USAGE","message":"build requires --source, --reference, and --output","details":null}
```
