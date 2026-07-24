# Frozen upstream compatibility corpus

The maintainer-only inspector validates the small checked Pangolin 1.0.2
GRCh38 corpus entirely offline. It does not load Python, model checkpoints, a
whole reference, or an annotation database.

```bash
pangopup-build compatibility inspect --corpus ../tests/fixtures/pangolin-compat-v1 | mustmatch like '{"status":"valid","schema":"pangopup-compat-v1","profile":"pangolin-1.0.2-5cf94b8-grch38-v1","cases":24,"scored_cases":14,"rejection_cases":6,"postprocess_cases":4,"coverage_cells":28}'
```

A missing corpus is a sanitized bounded I/O failure.

```bash run id=compatibility-missing exit=1 stream=stderr
pangopup-build compatibility inspect --corpus ../target/spec/no-such-compatibility-corpus
```

```text expect=compatibility-missing contains
{"status":"error","code":"IO","message":"corpus: No such file or directory
```

The command grammar is closed.

```bash run id=compatibility-usage exit=2 stream=stderr
pangopup-build compatibility inspect
```

```text expect=compatibility-usage exact
{"status":"error","code":"CLI_USAGE","message":"compatibility inspect requires --corpus exactly once","details":null}
```

A copied corpus with a hash-rebound semantic score mutation fails closed; no
model is run and no success summary is emitted. Rebinding the declared member
digest proves the inspector reached independent semantic replay instead of
stopping at file integrity.

```bash run id=compatibility-corrupt exit=1 stream=stderr
rm -rf ../target/spec/upstream-compatibility-corrupt
cp -a ../tests/fixtures/pangolin-compat-v1 ../target/spec/upstream-compatibility-corrupt
perl -pi -e 'if (/M02-snv-wrap53-tp53-precomputed/) { s/"gain_position":18/"gain_position":19/ }' ../target/spec/upstream-compatibility-corrupt/cases.jsonl
cases_sha=$(sha256sum ../target/spec/upstream-compatibility-corrupt/cases.jsonl | cut -d' ' -f1)
perl -pi -e "s/2aa557fd3b137966721d47ce073b2954c6a0bb1a6a64e9c4933dac69e88042c8/$cases_sha/" ../target/spec/upstream-compatibility-corrupt/manifest.json
pangopup-build compatibility inspect --corpus ../target/spec/upstream-compatibility-corrupt
```

```text expect=compatibility-corrupt exact
{"status":"error","code":"COMPATIBILITY_INVALID","message":"cases.jsonl:M02-snv-wrap53-tp53-precomputed: semantic score replay mismatch","details":null}
```
