# CLI identity

Pangopup starts as a command-line tool so the index contract can be tested and
benchmarked without a network layer. The walking skeleton identifies the exact
binary under test:

```bash
pangopup --version | mustmatch like "pangopup 0.3.0"
pangopup -V | mustmatch like "pangopup 0.3.0"
```

Root help exposes the exact local-assets and lookup grammar, while every
runtime path provides focused help without opening assets:

```bash
pangopup | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup -h | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup --help | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup --help | rg -F 'pangopup uninstall [--full] [--yes]' | mustmatch like '  pangopup uninstall [--full] [--yes]'
pangopup --help | rg -F 'pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup status [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup status [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup serve [--listen <ADDRESS>]' | mustmatch like '  pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup --help | rg -F 'pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON>' | mustmatch like '  pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --help | head -1 | mustmatch like 'Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --version | mustmatch like "pangopup 0.3.0"
```

The nine non-root leaf and namespace paths accept both conventional help
flags. Invalid asset environment values do not matter because help dispatches
before path resolution.

```bash
for flag in -h --help; do
  pangopup uninstall "$flag" | head -1
  pangopup sync "$flag" | head -1
  pangopup status "$flag" | head -1
  pangopup serve "$flag" | head -1
  pangopup assets "$flag" | head -1
  pangopup assets install "$flag" | head -1
  pangopup assets runtime "$flag" | head -1
  pangopup assets runtime install "$flag" | head -1
  PANGOPUP_DATA_DIR=relative PANGOPUP_CACHE_DIR=relative pangopup lookup "$flag" | head -1
done | mustmatch like "Usage: pangopup uninstall [--full] [--yes]
Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup assets <ACTION>
Usage: pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup assets runtime <ACTION>
Usage: pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup uninstall [--full] [--yes]
Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup assets <ACTION>
Usage: pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup assets runtime <ACTION>
Usage: pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]"
```

Uninstall tests use isolated executable copies and isolated roots. They never
invoke uninstall through the build-tree executable or touch user paths. The
noninteractive forms keep the checked path plan on stderr and emit exactly one
JSON result on stdout.

```bash
set -eu
root=../target/spec/uninstall-code
rm -rf "$root"
mkdir -p "$root/bin" "$root/data/pangopup" "$root/cache/pangopup"
cp ../target/debug/pangopup "$root/bin/pangopup"
PANGOPUP_DATA_DIR="$(cd "$root/data" && pwd)/pangopup" \
PANGOPUP_CACHE_DIR="$(cd "$root/cache" && pwd)/pangopup" \
  "$root/bin/pangopup" uninstall --yes >"$root/out" 2>"$root/err"
test ! -e "$root/bin/pangopup"
test -d "$root/data/pangopup"
test -d "$root/cache/pangopup"
test "$(wc -l < "$root/out")" -eq 1
rg -F '"status":"removed","scope":"code_only"' "$root/out" >/dev/null
rg -F 'Pangopup uninstall plan:' "$root/err" >/dev/null
printf 'isolated code-only uninstall passed\n' | mustmatch like 'isolated code-only uninstall passed'
```

```bash
set -eu
root=../target/spec/uninstall-full
rm -rf "$root"
mkdir -p "$root/bin" "$root/data/pangopup/bundles/example" "$root/cache/pangopup/profiles/example"
cp ../target/debug/pangopup "$root/bin/pangopup"
printf fixture >"$root/data/pangopup/bundles/example/member"
printf fixture >"$root/cache/pangopup/profiles/example/member"
PANGOPUP_DATA_DIR="$(cd "$root/data" && pwd)/pangopup" \
PANGOPUP_CACHE_DIR="$(cd "$root/cache" && pwd)/pangopup" \
  "$root/bin/pangopup" uninstall --full --yes >"$root/out" 2>"$root/err"
test ! -e "$root/bin/pangopup"
test ! -e "$root/data/pangopup"
test ! -e "$root/cache/pangopup"
test "$(wc -l < "$root/out")" -eq 1
rg -F '"status":"removed","scope":"full"' "$root/out" >/dev/null
printf 'isolated full uninstall passed\n' | mustmatch like 'isolated full uninstall passed'
```

Interactive code-only, full, preselected-full confirmation, and cancellation
run inside a disposable pseudo-terminal. Cancellation leaves the executable
and empty roots untouched.

```bash
set -eu
for case in code full confirm cancel; do
  root="../target/spec/uninstall-interactive-$case"
  rm -rf "$root"
  mkdir -p "$root/bin" "$root/data/pangopup" "$root/cache/pangopup"
  cp ../target/debug/pangopup "$root/bin/pangopup"
  data="$(cd "$root/data" && pwd)/pangopup"
  cache="$(cd "$root/cache" && pwd)/pangopup"
  command="env PANGOPUP_DATA_DIR=$data PANGOPUP_CACHE_DIR=$cache $root/bin/pangopup uninstall"
  answer=1
  test "$case" != full || answer=2
  test "$case" != cancel || answer=3
  if test "$case" = confirm; then command="$command --full"; answer=yes; fi
  printf '%s\n' "$answer" | script -q -e -c "$command" "$root/transcript" >/dev/null
done
test ! -e ../target/spec/uninstall-interactive-code/bin/pangopup
test ! -e ../target/spec/uninstall-interactive-full/bin/pangopup
test ! -e ../target/spec/uninstall-interactive-confirm/bin/pangopup
test -e ../target/spec/uninstall-interactive-cancel/bin/pangopup
test -d ../target/spec/uninstall-interactive-cancel/data/pangopup
test -d ../target/spec/uninstall-interactive-cancel/cache/pangopup
rg -F '"status":"cancelled","scope":"none"' ../target/spec/uninstall-interactive-cancel/transcript >/dev/null
printf 'isolated interactive uninstall choices passed\n' | mustmatch like 'isolated interactive uninstall choices passed'
```

Without `--yes`, redirected input is rejected before mutation. Unknown,
duplicate, and valued flags retain the ordinary usage envelope.

```bash run id=uninstall-nonterminal exit=2 stream=stderr
root=../target/spec/uninstall-nonterminal
rm -rf "$root"
mkdir -p "$root/bin" "$root/data/pangopup" "$root/cache/pangopup"
cp ../target/debug/pangopup "$root/bin/pangopup"
PANGOPUP_DATA_DIR="$(cd "$root/data" && pwd)/pangopup" PANGOPUP_CACHE_DIR="$(cd "$root/cache" && pwd)/pangopup" "$root/bin/pangopup" uninstall </dev/null
```

```text expect=uninstall-nonterminal contains
{"status":"error","code":"UNINSTALL_NONINTERACTIVE"
```

```bash run id=uninstall-unknown exit=2 stream=stderr
pangopup uninstall --full=yes
```

```text expect=uninstall-unknown contains
{"status":"error","code":"CLI_USAGE"
```

Misplaced, mixed, extended, and unknown help forms retain ordinary typed usage
failure instead of hiding malformed operations.

```bash run id=misplaced-help exit=2 stream=stderr
pangopup --help sync
```

```text expect=misplaced-help contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=mixed-help exit=2 stream=stderr
pangopup sync --offline --help
```

```text expect=mixed-help contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=extended-help exit=2 stream=stderr
pangopup assets runtime install --help extra
```

```text expect=extended-help contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=unknown-help exit=2 stream=stderr
pangopup unknown --help
```

```text expect=unknown-help contains
{"status":"error","code":"CLI_USAGE"
```

The former nested synchronization and status commands are rejected rather
than retained as aliases.

```bash run id=old-assets-sync exit=2 stream=stderr
pangopup assets sync --offline
```

```text expect=old-assets-sync contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=old-assets-status exit=2 stream=stderr
pangopup assets status
```

```text expect=old-assets-status contains
{"status":"error","code":"CLI_USAGE"
```

```bash run id=old-runtime-status exit=2 stream=stderr
pangopup assets runtime status
```

```text expect=old-runtime-status contains
{"status":"error","code":"CLI_USAGE"
```
