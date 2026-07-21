# CLI identity

Pangopup starts as a command-line tool so the index contract can be tested and
benchmarked without a network layer. The walking skeleton identifies the exact
binary under test:

```bash
pangopup --version | mustmatch like "pangopup 0.1.0"
```

Until the first lookup slice lands, help says plainly that score lookup is not
implemented:

```bash
pangopup --help | mustmatch like "Score lookup is not implemented yet."
```
