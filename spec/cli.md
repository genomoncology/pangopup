# CLI identity

Pangopup starts as a command-line tool so the index contract can be tested and
benchmarked without a network layer. The walking skeleton identifies the exact
binary under test:

```bash
pangopup --version | mustmatch like "pangopup 0.2.0"
```

Root help exposes the exact local-assets and lookup grammar, while every
runtime path provides focused help without opening assets:

```bash
pangopup | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup -h | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup --help | cmp - ../tests/fixtures/runtime-cli/root-help.txt
pangopup --help | rg -F 'pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup status [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup status [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup serve [--listen <ADDRESS>]' | mustmatch like '  pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup --help | rg -F 'pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON>' | mustmatch like '  pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --help | head -1 | mustmatch like 'Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --version | mustmatch like "pangopup 0.2.0"
```

The eight non-root leaf and namespace paths accept both conventional help
flags. Invalid asset environment values do not matter because help dispatches
before path resolution.

```bash
for flag in -h --help; do
  pangopup sync "$flag" | head -1
  pangopup status "$flag" | head -1
  pangopup serve "$flag" | head -1
  pangopup assets "$flag" | head -1
  pangopup assets install "$flag" | head -1
  pangopup assets runtime "$flag" | head -1
  pangopup assets runtime install "$flag" | head -1
  PANGOPUP_DATA_DIR=relative PANGOPUP_CACHE_DIR=relative pangopup lookup "$flag" | head -1
done | mustmatch like "Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup assets <ACTION>
Usage: pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup assets runtime <ACTION>
Usage: pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup sync [--offline] [--progress | --quiet] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]
Usage: pangopup status [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup serve [--listen <ADDRESS>] [--data-dir <ABSOLUTE_PATH>] [--model-workers <1..8>] [--model-threads <1..8>] [--model-queue-capacity <1..1024>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]
Usage: pangopup assets <ACTION>
Usage: pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup assets runtime <ACTION>
Usage: pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]
Usage: pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] [--model-only] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]"
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
