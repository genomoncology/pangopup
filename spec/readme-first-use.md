# README first-use contract

The root README is a compact user guide. These checks inspect text only; they
do not use the network, synchronize assets, start a service, or remove files.

```bash
test "$(wc -l < ../README.md)" -le 220
test "$(wc -w < ../README.md)" -le 1700
test "$(sed -n '1p' ../README.md)" = '# PangoPup'
headings=$(rg '^## ' ../README.md)
test "$headings" = "$(printf '%s\n' \
  '## Quick start' \
  '## Input and output' \
  '## HTTP service' \
  '## Docker' \
  '## Storage and operations' \
  '## Citation and license')"
printf 'README structure is compact and user-first\n' | mustmatch like 'README structure is compact and user-first'
```

The title is followed directly by the product and scientific explanation.

```bash
opening=$(awk 'NR == 1 { next } /^## / { exit } { print }' ../README.md)
for text in \
  'fast, local' \
  'Pangolin' \
  'GRCh38 variants' \
  'strongest predicted splice-site' \
  'gain and signed loss' \
  'genomic-coordinate' \
  'memory-mapped index' \
  'runs through the Pangolin model' \
  'saved in SQLite for reuse' \
  '50 bases on either side' \
  'deletion'; do
  printf '%s' "$opening" | rg -F "$text" >/dev/null
done
! rg -n '^## (Introduction|What it predicts|What it does not do|Limitations)' ../README.md
printf 'opening explains the product directly\n' | mustmatch like 'opening explains the product directly'
```

Quick start contains one complete direct-CLI path and no competing modality.

```bash
quick=$(awk '/^## Quick start$/ { on=1; next } /^## / { if (on) exit } on' ../README.md)
for text in \
  'Linux x86-64/amd64 with GLIBC 2.39 or newer' \
  '2.44 GiB' \
  '14.76 GiB' \
  '25 GB free' \
  'raw.githubusercontent.com/genomoncology/pangopup/v0.3.0/install.sh' \
  'bash -s -- --version 0.3.0' \
  'export PATH="$HOME/.local/bin:$PATH"' \
  'pangopup sync --progress' \
  'pangopup status' \
  'pangopup lookup --variant GRCh38:chr12:6801301:G:A' \
  'pangopup lookup --variant GRCh38:chr12:6801303:G:GA' \
  'JSON Lines' \
  'network-free'; do
  printf '%s' "$quick" | rg -F "$text" >/dev/null
done
! printf '%s' "$quick" | rg -i 'docker|pangopup serve|/v1/score|git clone|cargo build' >/dev/null
printf 'CLI quick start is complete and bounded\n' | mustmatch like 'CLI quick start is complete and bounded'
```

Input and output rules stay with the CLI examples they govern.

```bash
io=$(awk '/^## Input and output$/ { on=1; next } /^## / { if (on) exit } on' ../README.md)
for text in \
  'GRCh38:CONTIG:POS:REF:ALT' \
  '1-based genomic position' \
  '1`–`22`, `X`, `Y`, `M`' \
  'RefSeq accessions' \
  'uppercase strings' \
  'does not trim, align, or' \
  'anchored form' \
  'share the first base' \
  'checks REF against' \
  'at most 100 bases' \
  '--format table' \
  '--gene ENSG00000010610' \
  '--model-only' \
  'JSON Lines is the default' \
  '`status`' \
  '`found`' \
  'at least one score record' \
  'no source-reference ambiguity' \
  '`not_found`' \
  'not a prediction of zero effect' \
  '`ambiguous_source_reference`' \
  'used `N` as its reference' \
  'source-associated gene, published alternate alleles, and omitted' \
  '`mixed`' \
  'both occurred' \
  'multiple records' \
  '`gain_score`' \
  '`loss_score`' \
  'Loss is signed' \
  'higher genomic coordinate' \
  '`provenance.kind`' \
  '`precomputed`' \
  '`model`'; do
  printf '%s' "$io" | rg -F -- "$text" >/dev/null
done
printf 'input and output contract is discoverable\n' | mustmatch like 'input and output contract is discoverable'
```

HTTP instructions contain a runnable foreground path, readiness check,
scoring request, admission limits, and exposure guidance.

```bash
http=$(awk '/^## HTTP service$/ { on=1; next } /^## / { if (on) exit } on' ../README.md)
for text in \
  'pangopup serve --listen 127.0.0.1:8080' \
  '/livez' \
  '/readyz' \
  '/v1/status' \
  '/v1/score' \
  'content-type: application/json' \
  '"model_only":true' \
  '1–100 variants' \
  '10 uncached model variants' \
  'HTTP 429' \
  'no built-in authentication or TLS' \
  'authenticated TLS reverse proxy'; do
  printf '%s' "$http" | rg -F "$text" >/dev/null
done
printf 'HTTP first-use contract is complete\n' | mustmatch like 'HTTP first-use contract is complete'
```

Docker offers one persistent sync/service path and one direct CLI call.

```bash
docker=$(awk '/^## Docker$/ { on=1; next } /^## / { if (on) exit } on' ../README.md)
for text in \
  'ghcr.io/genomoncology/pangopup:0.3.0' \
  'docker volume create pangopup-data' \
  'docker volume create pangopup-cache' \
  'pangopup-data:/var/lib/pangopup:ro' \
  'pangopup-cache:/var/cache/pangopup' \
  'sync --progress' \
  'lookup --variant GRCh38:chr12:6801301:G:A' \
  'preserves both named volumes' \
  'Apple Silicon' \
  'CPU-only' \
  'does not use MPS or' ; do
  printf '%s' "$docker" | rg -F "$text" >/dev/null
done
printf 'Docker path is coherent\n' | mustmatch like 'Docker path is coherent'
```

Storage and operations retain only capacity, mmap, XDG, offline, update, and
removal guidance useful to operators.

```bash
ops=$(awk '/^## Storage and operations$/ { on=1; next } /^## / { if (on) exit } on' ../README.md)
for text in \
  'SNV lookup' \
  '~1.80 GiB' \
  '~14.00 GiB' \
  '~660 MiB' \
  '~775 MiB' \
  '~2.44 GiB' \
  '~14.76 GiB' \
  'memory-mapped rather than loaded wholly into RAM' \
  '256 MiB RAM' \
  '~/.local/share/pangopup' \
  '~/.cache/pangopup/model-results.sqlite3' \
  'pangopup sync --offline' \
  'VERSION=0.3.0' \
  'v${VERSION}/install.sh' \
  'preserving assets and caches' \
  'pangopup sync --progress' \
  'docker pull "$PANGOPUP_IMAGE"' \
  'docker stop pangopup' \
  'preserves the data and cache volumes' \
  'pangopup uninstall --yes' \
  'pangopup uninstall --full --yes' \
  'docker volume rm pangopup-data'; do
  printf '%s' "$ops" | rg -F -- "$text" >/dev/null
done
printf 'storage and lifecycle guidance is compact\n' | mustmatch like 'storage and lifecycle guidance is compact'
```

Attribution is concise and exact.

```bash
citation=$(awk '/^## Citation and license$/ { on=1; next } on' ../README.md)
for text in \
  '[`CITATION.cff`](CITATION.cff)' \
  'GPL-3.0-only' \
  'Tony Zeng' \
  'Yang I. Li' \
  'github.com/tkzeng/Pangolin' \
  'link.springer.com/article/10.1186/s13059-022-02664-4' \
  '10.5281/zenodo.15649338' \
  'CC BY 4.0' \
  'GENCODE v38' \
  '[`NOTICE`](NOTICE)'; do
  printf '%s' "$citation" | rg -F "$text" >/dev/null
done
printf 'citation facts are present\n' | mustmatch like 'citation facts are present'
```

Internal history, maintainer instructions, and non-feature trivia stay out of
the root user guide.

```bash
for phrase in \
  'v0.2.0' \
  'immutable release' \
  'release-engineering' \
  'HGVS' \
  'pathogenicity classification' \
  'clinical diagnosis' \
  '## Platform and service limits' \
  'fixed-width records' \
  'PSS/RSS' \
  'warm-page-cache' \
  'git rev-parse' \
  'cpuinfo_vendor' \
  'Apple-aware' \
  'pangopup-build --help' \
  'make lint' \
  'make test' \
  'make spec'; do
  ! rg -F "$phrase" ../README.md >/dev/null
done
! rg -n '\]\((planning/|AGENTS\.md)' ../README.md
printf 'internal trivia is absent\n' | mustmatch like 'internal trivia is absent'
```
