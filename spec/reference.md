# Production reference maintenance surface

The normal acceptance path builds only the registered 25-contig synthetic
profile. It uses the production `PGRREF01` container and private exhaustive
certification without reading the full NCBI source.

```bash
rm -rf ../target/spec/reference-production
mkdir -p ../target/spec/reference-production
pangopup-build reference build --profile pangopup-reference-mini-v1 --source ../tests/fixtures/reference-production-mini/source.fa.gz --assembly-report ../tests/fixtures/reference-production-mini/assembly_report.txt --output ../target/spec/reference-production/bundle | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' | mustmatch like '{"bundle_id":"sha256:<digest>","certification":{"contexts_verified":4,"sequence_set_sha256":"sha256:<digest>","total_bases":159},"command":"reference.build","members":[{"path":"NOTICE","sha256":"sha256:<digest>","size":279},{"path":"reference.pgr","sha256":"sha256:<digest>","size":4560}],"ok":true,"profile":"pangopup-reference-mini-v1"}'
pangopup-build reference inspect --bundle ../target/spec/reference-production/bundle | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' | mustmatch like '{"bundle_id":"sha256:<digest>","command":"reference.inspect","format":"pangopup.reference.acgt2-rle.v1","integrity":"structural_only","member_sha256_checked":false,"ok":true,"profile":"pangopup-reference-mini-v1","sequences":25,"total_bases":159}'
find ../target/spec/reference-production/bundle -mindepth 1 -maxdepth 1 -type f -printf '%f\n' | sort | mustmatch like "NOTICE
manifest.json
reference.pgr"
```

The window command accepts a versioned RefSeq accession, resolves it to the
canonical contig, and returns exact uppercase IUPAC bases with bundle
provenance.

```bash
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig NC_000001.11 --start 1 --length 15 | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/g' | mustmatch like '{"bases":"ACGTRYSWKMBDHVN","command":"reference.window","contig":"chr1","length":15,"ok":true,"provenance":{"assembly":"synthetic-mini","assembly_accession":"pangopup-reference-mini-v1","bundle_id":"sha256:<digest>","format":"pangopup.reference.acgt2-rle.v1","profile":"pangopup-reference-mini-v1","sequence_set_sha256":"sha256:<digest>"},"start":1}'
```

Usage and operational failures both emit one canonical JSON line on standard
output, leave standard error empty, and do not disclose supplied paths.

```bash run id=reference-window-usage exit=2 stream=stdout
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig chrMT --start 1 --length 10
```

```text expect=reference-window-usage exact
{"command":"reference.window","error":{"code":"CLI_USAGE","message":"reference contig is invalid"},"ok":false}
```

```bash run id=reference-window-range exit=1 stream=stdout
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig chrM --start 8 --length 3
```

```text expect=reference-window-range exact
{"command":"reference.window","error":{"code":"REFERENCE_WINDOW","message":"reference window is invalid"},"ok":false}
```

```bash run id=reference-existing exit=1 stream=stdout
pangopup-build reference build --profile pangopup-reference-mini-v1 --source ../tests/fixtures/reference-production-mini/source.fa --assembly-report ../tests/fixtures/reference-production-mini/assembly_report.txt --output ../target/spec/reference-production/bundle
```

```text expect=reference-existing exact
{"command":"reference.build","error":{"code":"ALREADY_EXISTS","message":"reference output already exists"},"ok":false}
```

The remaining grammar and redaction controls reject missing, duplicate,
unknown, nonnumeric, and oversized arguments before touching data. Operational
errors expose neither the supplied path nor an operating-system message.

```bash run id=reference-duplicate-flag exit=2 stream=stdout
pangopup-build reference inspect --bundle ../target/spec/reference-production/bundle --bundle /secret/duplicate
```

```text expect=reference-duplicate-flag exact
{"command":"reference.inspect","error":{"code":"CLI_USAGE","message":"reference inspect arguments are invalid"},"ok":false}
```

```bash run id=reference-unknown-flag exit=2 stream=stdout
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig chr1 --start 1 --unknown 1
```

```text expect=reference-unknown-flag exact
{"command":"reference.window","error":{"code":"CLI_USAGE","message":"reference window arguments are invalid"},"ok":false}
```

```bash run id=reference-nonnumeric exit=2 stream=stdout
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig chr1 --start nope --length 1
```

```text expect=reference-nonnumeric exact
{"command":"reference.window","error":{"code":"CLI_USAGE","message":"reference start is invalid"},"ok":false}
```

```bash run id=reference-oversized exit=2 stream=stdout
pangopup-build reference window --bundle ../target/spec/reference-production/bundle --contig chr1 --start 1 --length 1048577
```

```text expect=reference-oversized exact
{"command":"reference.window","error":{"code":"CLI_USAGE","message":"reference length is invalid"},"ok":false}
```

```bash run id=reference-inspect-redacted exit=1 stream=stdout
pangopup-build reference inspect --bundle /secret/pangopup-do-not-disclose
```

```text expect=reference-inspect-redacted exact
{"command":"reference.inspect","error":{"code":"REFERENCE_BUNDLE","message":"reference bundle is invalid"},"ok":false}
```

```bash run id=reference-build-redacted exit=1 stream=stdout
pangopup-build reference build --profile pangopup-reference-mini-v1 --source /secret/pangopup-source-do-not-disclose --assembly-report /secret/pangopup-report-do-not-disclose --output ../target/spec/reference-production/redacted
```

```text expect=reference-build-redacted exact
{"command":"reference.build","error":{"code":"REFERENCE_INPUT","message":"reference input is invalid"},"ok":false}
```
