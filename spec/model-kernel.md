# Authenticated raw CPU model kernel

The checked miniature uses the production three-file bundle grammar and opens
through the real pinned ONNX Runtime CPU path. Inspection authenticates all
members but does not claim variant-level scoring.

```bash
pangopup-build model inspect --bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle
```

```text
{"bundle_id":"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca","channels":12,"checkpoints":0,"command":"model.inspect","kind":"synthetic-test","model_bytes":281,"notice_bytes":222,"profile":"pangopup-model-kernel-mini-v1","schema":"pangopup-model-bundle-v1","status":"ok"}
```

Qualification executes four retained sequence/strand/allele inputs and all
twelve raw channels against an independently generated bit-pattern oracle.
Host strings are evidence but are normalized in this portable executable
contract.

```bash
pangopup-build model qualify --bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle --evidence ../tests/fixtures/pangolin-model-kernel-mini/evidence | sed -E 's/"cpu":"[^"]*"/"cpu":"<cpu>"/; s/"rustc":"[^"]*"/"rustc":"<rustc>"/'
```

```text
{"bundle_id":"sha256:aba3f0a07075f24cc5c3c59eb4312176bae4f2886db8946500280b19e686edca","cases":2,"channel_arrays":48,"command":"model.qualify","maximum_absolute_error":0,"profile":"pangopup-model-kernel-mini-v1","runtime":{"architecture":"x86_64","cpu":"<cpu>","execution_mode":"sequential","execution_provider":"CPUExecutionProvider","graph_optimization":"all","inter_op_threads":1,"intra_op_threads":1,"onnx_runtime":"1.24.2","ort_crate":"2.0.0-rc.12","rustc":"<rustc>"},"scalar_comparisons":816,"sequence_evaluations":4,"status":"ok","strands":2}
```

The model command grammar is closed. Missing actions, unknown actions, and
duplicate flags fail before opening supplied paths.

```bash run id=model-missing-action exit=2 stream=stderr
pangopup-build model
```

```text expect=model-missing-action exact
{"status":"error","code":"CLI_USAGE","message":"model requires evidence, convert, inspect, or qualify","details":null}
```

```bash run id=model-unknown-action exit=2 stream=stderr
pangopup-build model unknown --bundle /secret/not-opened
```

```text expect=model-unknown-action exact
{"status":"error","code":"CLI_USAGE","message":"model requires evidence, convert, inspect, or qualify","details":null}
```

```bash run id=model-duplicate-bundle exit=2 stream=stderr
pangopup-build model inspect --bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle --bundle /secret/not-opened
```

```text expect=model-duplicate-bundle exact
{"status":"error","code":"CLI_USAGE","message":"model inspect requires --bundle exactly once","details":null}
```

A missing directory and a structurally corrupt authenticated member fail with
one path-free typed JSON error.

```bash run id=model-missing-bundle exit=1 stream=stderr
pangopup-build model inspect --bundle /secret/pangopup-model-does-not-exist
```

```text expect=model-missing-bundle exact
{"code":"MODEL_BUNDLE","details":null,"message":"inspect bundle directory: No such file or directory (os error 2)","status":"error"}
```

```bash run id=model-corrupt-member exit=1 stream=stderr
rm -rf ../target/spec/model-kernel/corrupt
mkdir -p ../target/spec/model-kernel
cp -R ../tests/fixtures/pangolin-model-kernel-mini/bundle ../target/spec/model-kernel/corrupt
truncate -s 1 ../target/spec/model-kernel/corrupt/model.onnx
pangopup-build model inspect --bundle ../target/spec/model-kernel/corrupt
```

```text expect=model-corrupt-member exact
{"code":"MODEL_BUNDLE","details":null,"message":"invalid model bundle: member byte length","status":"error"}
```

Rebinding a changed golden byte and its member digest does not turn the
candidate into valid evidence. The real runtime result still disagrees with
the independently asserted oracle.

```bash run id=model-rebound-golden exit=1 stream=stderr
rm -rf ../target/spec/model-kernel/rebound
mkdir -p ../target/spec/model-kernel
cp -R ../tests/fixtures/pangolin-model-kernel-mini/evidence ../target/spec/model-kernel/rebound
old=$(sha256sum ../target/spec/model-kernel/rebound/kernel-golden.jsonl | cut -d' ' -f1)
sed -i '0,/0000803f/s//00000000/' ../target/spec/model-kernel/rebound/kernel-golden.jsonl
new=$(sha256sum ../target/spec/model-kernel/rebound/kernel-golden.jsonl | cut -d' ' -f1)
sed -i "s/$old/$new/" ../target/spec/model-kernel/rebound/manifest.json
pangopup-build model qualify --bundle ../tests/fixtures/pangolin-model-kernel-mini/bundle --evidence ../target/spec/model-kernel/rebound
```

```text expect=model-rebound-golden exact
{"code":"MODEL_QUALIFICATION","details":null,"message":"maximum absolute error 1 exceeds 0.00001","status":"error"}
```
