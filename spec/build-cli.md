# Maintainer CLI identity and discoverability

The maintenance executable reports its workspace version and derives its
complete command list from the same catalog used for dispatch.

```bash
workspace_version="$(sed -n 's/^version = "\([^"]*\)"$/\1/p' ../Cargo.toml)"
test -n "$workspace_version"
test "$(pangopup-build --version)" = "pangopup-build $workspace_version"
test "$(pangopup-build -V)" = "pangopup-build $workspace_version"
printf 'maintainer version follows the workspace package\n' | mustmatch like 'maintainer version follows the workspace package'
pangopup-build --help | sed -n 's/^  pangopup-build //p' | mustmatch like "inspect <SOURCE_DIR>
prototype-roundtrip <SOURCE_DIR> <OUTPUT>
prototype-open <ARTIFACT>
benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>
build --source <SOURCE_DIR> --reference <GRCH38_FASTA_OR_GZIP> --output <NEW_BUNDLE>
verify <BUNDLE>
reference build --profile <refseq-grch38p14-primary-v1|pangopup-reference-mini-v1|pangopup-reference-route-test-v1> --source <FASTA_OR_GZIP> --assembly-report <ASSEMBLY_REPORT> --output <NEW_BUNDLE>
reference inspect --bundle <BUNDLE>
reference window --bundle <BUNDLE> --contig <GRCH38_CONTIG_OR_REFSEQ_ACCESSION> --start <POSITIVE_1_BASED_POSITION> --length <1..1048576>
transport pack --bundle <BUNDLE> --output <ABSENT_DIR>
transport verify --transport <TRANSPORT_DIR>
transport unpack --transport <TRANSPORT_DIR> --output <ABSENT_DIR>
release prepare --transport <TRANSPORT_DIR> --receipt <PROOF_RECEIPT_JSON> --output <ABSENT_DIR>
release upload-asset --transport <TRANSPORT_DIR> --prepared <PREPARED_DIR> --gh <ABSOLUTE_PINNED_GH_BINARY> --release-id <POSITIVE_GITHUB_ID> --asset <EXACT_ASSET_NAME>
compatibility inspect --corpus <CORPUS_DIR>
compatibility capture --upstream <PANGOLIN_DIR> --python <PYTHON> --reference-source <REFSEQ_FASTA_GZIP> --assembly-report <ASSEMBLY_REPORT> --reference <DERIVED_FASTA> --annotation-db <GENCODE_DB> --annotation-gtf <GENCODE_GTF_GZIP> --output <ABSENT_DIR>
model evidence --upstream <PANGOLIN_DIR> --python <PYTHON> --corpus <CORPUS_DIR> --output <ABSENT_DIR>
model convert --upstream <PANGOLIN_DIR> --python <PYTHON> --evidence <EVIDENCE_DIR> --output <ABSENT_DIR> --representation <singleton|zero-padded-batch|paired-strand-batch>
model inspect --bundle <MODEL_BUNDLE>
model qualify --bundle <MODEL_BUNDLE> --evidence <EVIDENCE_DIR>
runtime-profile prepare --snv-bundle <SNV_BUNDLE> --model-bundle <MODEL_BUNDLE> --reference-bundle <REFERENCE_BUNDLE> --mask <MASK_FILE> --output <PROFILE_JSON>"
```

Short help, namespace help, and leaf help are successful stdout-only
information. Leaf help states the exact closed grammar without opening any
input or output.

```bash
pangopup-build -h | head -1 | mustmatch like "Usage: pangopup-build <COMMAND>"
pangopup-build reference --help | sed -n 's/^  pangopup-build //p' | mustmatch like "reference build --profile <refseq-grch38p14-primary-v1|pangopup-reference-mini-v1|pangopup-reference-route-test-v1> --source <FASTA_OR_GZIP> --assembly-report <ASSEMBLY_REPORT> --output <NEW_BUNDLE>
reference inspect --bundle <BUNDLE>
reference window --bundle <BUNDLE> --contig <GRCH38_CONTIG_OR_REFSEQ_ACCESSION> --start <POSITIVE_1_BASED_POSITION> --length <1..1048576>"
pangopup-build model convert -h | mustmatch like "Usage: pangopup-build model convert --upstream <PANGOLIN_DIR> --python <PYTHON> --evidence <EVIDENCE_DIR> --output <ABSENT_DIR> --representation <singleton|zero-padded-batch|paired-strand-batch>

Convert authenticated Pangolin checkpoints into an ONNX bundle."
```

Every cataloged leaf has an informational path. Exercising all of them in an
empty directory creates nothing and emits nothing on standard error.

```bash
rm -rf ../target/spec/build-cli
mkdir -p ../target/spec/build-cli/work
cd ../target/spec/build-cli/work
while IFS= read -r path; do
  output="$(pangopup-build $path --help 2>help.stderr)"
  test -n "$output"
  test ! -s help.stderr
done <<'EOF'
inspect
prototype-roundtrip
prototype-open
benchmark-corpus
build
verify
reference build
reference inspect
reference window
transport pack
transport verify
transport unpack
release prepare
release upload-asset
compatibility inspect
compatibility capture
model evidence
model convert
model inspect
model qualify
runtime-profile prepare
EOF
rm help.stderr
test -z "$(find . -mindepth 1 -print -quit)"
printf 'all maintainer help paths are side-effect free\n' | mustmatch like 'all maintainer help paths are side-effect free'
```

Help is informational only in the exact accepted positions. Misplaced or
extended help remains ordinary operational input and cannot bypass the
established compact JSON errors.

```bash run id=build-help-misplaced exit=2 stream=stderr
pangopup-build --help reference
```

```text expect=build-help-misplaced exact
{"status":"error","code":"CLI_USAGE","message":"Usage: pangopup-build inspect <SOURCE_DIR>\n       pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>\n       pangopup-build prototype-open <ARTIFACT>\n       pangopup-build benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>","details":null}
```

```bash run id=build-help-extended exit=2 stream=stdout
pangopup-build reference inspect --help extra
```

```text expect=build-help-extended exact
{"command":"reference.inspect","error":{"code":"CLI_USAGE","message":"reference inspect arguments are invalid"},"ok":false}
```

Representative preexisting no-argument, partial, duplicate, and operational
errors retain both their exact bytes and their historical output stream.

```bash run id=build-cli-no-argument exit=2 stream=stderr
pangopup-build
```

```text expect=build-cli-no-argument exact
{"status":"error","code":"CLI_USAGE","message":"Usage: pangopup-build inspect <SOURCE_DIR>\n       pangopup-build prototype-roundtrip <SOURCE_DIR> <OUTPUT>\n       pangopup-build prototype-open <ARTIFACT>\n       pangopup-build benchmark-corpus <SOURCE_DIR> <OUTPUT> <SELECTED_MANIFEST>","details":null}
```

```bash run id=build-cli-partial exit=2 stream=stderr
pangopup-build build --source /secret/not-opened
```

```text expect=build-cli-partial exact
{"status":"error","code":"CLI_USAGE","message":"build requires --source, --reference, and --output","details":null}
```

```bash run id=build-cli-duplicate exit=2 stream=stderr
pangopup-build model inspect --bundle /secret/a --bundle /secret/b
```

```text expect=build-cli-duplicate exact
{"status":"error","code":"CLI_USAGE","message":"model inspect requires --bundle exactly once","details":null}
```

```bash run id=build-cli-reference-operational exit=1 stream=stdout
pangopup-build reference inspect --bundle /secret/not-opened
```

```text expect=build-cli-reference-operational exact
{"command":"reference.inspect","error":{"code":"REFERENCE_BUNDLE","message":"reference bundle is invalid"},"ok":false}
```
