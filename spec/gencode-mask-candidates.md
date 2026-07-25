# Private GENCODE mask candidate qualification

The feature-gated maintenance executable publishes its complete closed grammar
without reading Python, gffutils, the production annotation database, or any
network resource.

```bash
pangopup-mask-candidates --help | mustmatch like '{"command":"help","help":"usage: pangopup-mask-candidates <command> [options]\ncommands:\ncapture --database ABS --gtf ABS --python ABS --python-bytes N --python-sha256 HEX --python-launcher ABS --python-launcher-link-bytes N --python-launcher-link-sha256 HEX --pyvenv-config-bytes N --pyvenv-config-sha256 HEX --output-parent ABS\nprepare --stage ABS --compatibility-corpus ABS\ninspect --stage ABS\nquery --stage ABS --candidate interval-tree|domains|binned-postings --contig CONTIG --position N [--gene ENSG...]\nbenchmark --stage ABS\nreuse --prior-stage ABS --output-parent ABS --authorization ABS\nplan-capture-promotion --prior-stage ABS --source-builder-sha256 HEX\npromote-capture --prior-stage ABS --output-parent ABS --authorization ABS\nall successful commands emit one canonical JSON line; failures emit one sanitized JSON line","ok":true}'
```

The grammar rejects an incomplete point query on standard output with one
bounded JSON object and keeps standard error empty.

```bash run id=mask-candidate-usage exit=2 stream=stdout
pangopup-mask-candidates query --stage /tmp/not-opened --candidate domains --contig chr1
```

```text expect=mask-candidate-usage exact
{"code":"USAGE","command":"query","message":"required option is missing","ok":false}
```

The documented capture shell preserves the command's status while printing its
structured failure before exiting. Command substitution therefore cannot hide
the only immediate diagnostic.

```bash run id=mask-capture-shell-failure exit=2 stream=stdout
CAPTURE_JSON=''
if CAPTURE_JSON="$(pangopup-mask-candidates capture)"; then
  printf '%s\n' "$CAPTURE_JSON"
else
  status=$?
  printf '%s\n' "$CAPTURE_JSON"
  exit "$status"
fi
```

```text expect=mask-capture-shell-failure exact
{"code":"USAGE","command":"capture","message":"required option is missing","ok":false}
```

The read-only promotion planner and the separately authorized promotion command
also expose closed grammars. Neither silently falls back to ordinary reuse.

```bash run id=mask-promotion-usage exit=2 stream=stdout
pangopup-mask-candidates promote-capture --prior-stage /tmp/not-opened
```

```text expect=mask-promotion-usage exact
{"code":"USAGE","command":"promote-capture","message":"required option is missing","ok":false}
```
