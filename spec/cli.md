# CLI identity

Pangopup starts as a command-line tool so the index contract can be tested and
benchmarked without a network layer. The walking skeleton identifies the exact
binary under test:

```bash
pangopup --version | mustmatch like "pangopup 0.1.0"
```

Help exposes the exact lookup grammar:

```bash
pangopup --help | rg -F 'pangopup lookup --bundle <PATH> --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT>' | mustmatch like '  pangopup lookup --bundle <PATH> --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table]'
pangopup lookup --help | rg -F 'pangopup lookup --bundle <PATH> --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT>' | mustmatch like '  pangopup lookup --bundle <PATH> --variant GRCh38:<CONTIG>:<POS>:<REF>:<ALT> [--variant ...] [--gene <ENSG>] [--format jsonl|table]'
pangopup lookup --version | mustmatch like "pangopup 0.1.0"
```
