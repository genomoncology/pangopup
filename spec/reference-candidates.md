# Checked reference-format candidates

The maintenance inspector authenticates the complete registered miniature
candidate set, validates all three containers, and compares each decoder with
literal source expectations. It does not use FASTA, Python, Pangolin, model
weights, or a network request.

```bash
pangopup-build reference-candidates inspect --candidates ../tests/fixtures/reference-candidates-mini/candidates --corpus ../tests/fixtures/reference-candidates-mini/corpus | mustmatch like '{"candidate_set_sha256":"557cfa37dda0cb7d89b552d2e3cb2a3c31ebea26a937f386ff149e6ed17c08ff","command":"reference-candidates.inspect","contexts_verified":5,"corpus_manifest_sha256":"528a7b736112b3188ce9ab31fdbdbe14052e8ba5388433f3df7f18f82236474e","members":[{"bytes":20499,"codec":"ascii8"},{"bytes":12298,"codec":"iupac4"},{"bytes":8416,"codec":"acgt2-rle-v1"}],"ok":true,"profile":"pangopup-reference-candidates-mini-v1","source_sha256":"57d45eee6e9c14b2ca170b7ac3014dd45100d797f4a763f08d5abc8a6a4fb1c8"}'
```

The grammar is closed, emits one JSON line on standard output, keeps standard
error empty, and distinguishes usage from operational failure.

```bash run id=reference-candidate-usage exit=2 stream=stdout
pangopup-build reference-candidates inspect --corpus ../tests/fixtures/reference-candidates-mini/corpus
```

```text expect=reference-candidate-usage exact
{"command":"reference-candidates.inspect","error":{"code":"usage","message":"inspect requires candidates and corpus"},"ok":false}
```

A copied set whose payload changed cannot become trusted by retaining its old
manifest.

```bash run id=reference-candidate-corrupt exit=1 stream=stdout
rm -rf ../target/spec/reference-candidate-corrupt
cp -a ../tests/fixtures/reference-candidates-mini/candidates ../target/spec/reference-candidate-corrupt
printf X | dd of=../target/spec/reference-candidate-corrupt/iupac4.pgr bs=1 seek=4096 conv=notrunc status=none
pangopup-build reference-candidates inspect --candidates ../target/spec/reference-candidate-corrupt --corpus ../tests/fixtures/reference-candidates-mini/corpus
```

```text expect=reference-candidate-corrupt exact
{"command":"reference-candidates.inspect","error":{"code":"unsupported_profile","message":"miniature candidate identity mismatch"},"ok":false}
```
