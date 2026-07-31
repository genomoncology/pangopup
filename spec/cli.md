# CLI identity

Pangopup starts as a command-line tool so the index contract can be tested and
benchmarked without a network layer. The walking skeleton identifies the exact
binary under test:

```bash
pangopup --version | mustmatch like "pangopup 0.1.0"
```

Help exposes the exact local-assets and lookup grammar:

```bash
pangopup --help | rg -F 'pangopup sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup sync [--offline] [--data-dir <ABSOLUTE_PATH>] [--cache-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup status [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup status [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup assets install --transport <DIR> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON>' | mustmatch like '  pangopup assets runtime install --profile <CANONICAL_PROFILE_JSON> --model-bundle <DIR> --reference-bundle <DIR> --mask <FILE> [--data-dir <ABSOLUTE_PATH>]'
pangopup --help | rg -F 'pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --help | rg -F 'pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>]' | mustmatch like '  pangopup lookup [--bundle <DIR> | --data-dir <ABSOLUTE_PATH>] --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table] [--model-bundle <DIR> --reference-bundle <DIR> --mask <FILE>] [--model-cache <ABSOLUTE_PATH>] [--model-cache-max-entries <POSITIVE_INTEGER|unlimited>]'
pangopup lookup --version | mustmatch like "pangopup 0.1.0"
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
