---
---
# A CLI lookup pays the whole naming source on every invocation

`run_lookup_with_runtime_opener` (`crates/pangopup-cli/src/main.rs`) calls `installed_gene_names` as soon as it resolves a data root. That reads the published naming source and parses every row into a map before the command knows whether any record will be rendered, whether the format is `table`, or whether the source names any gene the lookup returns.

Measured on 2026-09-08 against the 2026-09-04 HGNC release, 16,903,161 bytes and 42,358 accessions, with a release build and a warm page cache. One `pangopup lookup --variant GRCh38:chr7:140753336:A:T` takes 6 ms with no naming source installed and 155 ms with one installed. The cost is the parse, and a single-variant invocation pays all of it.

The HTTP service does not have this problem. `serve` reads the naming source once at startup and every request reuses it.

Ticket 0039 did not name latency, so this was left standing at code review. Three shapes are available. The format guard skips the read for `--format table`, which the renderer ignores anyway. Lazy opening defers the read until the first record that needs a name. A parsed on-disk form removes the parse from every invocation. The first is one line and helps only table output. The third is the largest change and the only one that helps a single jsonl lookup reach its old latency.

The benchmark harnesses in `crates/pangopup-cli/benches/` pass `None` for the naming source, so none of them measures this.
