# Miniature gene-naming source

`hgnc_complete_set_2026-09-04.tsv` is a ten-row excerpt of the HGNC complete set, monthly release 2026-09-04. It keeps the full 53-column header so it parses exactly like the published file. It carries the published file name because it belongs to that release. Its own size and digest are its own. Only the fetched production file matches the published pin below.

Source: HGNC (HUGO Gene Nomenclature Committee), EMBL-EBI.
File: `hgnc_complete_set_2026-09-04.tsv`.
URL: <https://storage.googleapis.com/public-download-files/hgnc/archive/archive/monthly/tsv/hgnc_complete_set_2026-09-04.tsv>
Published size: 16903161 bytes.
Published SHA-256: `6f43d6ff43aa9fdfa5fb2f20a20a7cace66e6e02e2a0dcf19d9b726e2e248d20`.

HGNC publishes no upstream checksum file for this release. PangoPup pins it by byte count and its own computed digest.

Rows are sorted by Ensembl gene accession, then by HGNC identifier. Five accessions also appear in `../snv-regression/bundle`, so a score lookup against that bundle observes them.

HGNC quotes a field when it holds more than one pipe-separated value, and leaves a single value unquoted. `alias_symbol` for CD4 reads `"T4|Leu-3"` and `prev_symbol` for WRAP53 reads `WDR79`. The published release quotes 11,046 of its 45,045 `alias_symbol` cells. This fixture keeps that quoting verbatim. A parser that splits on `|` without stripping the surrounding quotes yields `"T4` and `Leu-3"`. The spec assertions in `spec/gene-naming.md` fail until the parser handles the quoting.

| Accession | Row | Why it is here |
| --- | --- | --- |
| `ENSG00000010610` | `CD4` | A named score-bundle gene with alias symbols and no previous symbol. |
| `ENSG00000119888` | `EPCAM` | The maximum multiplicity case: 3 previous symbols and 22 alias symbols. |
| `ENSG00000141499` | `WRAP53` | A named score-bundle gene carrying both a previous symbol and alias symbols. |
| `ENSG00000141510` | `TP53` | Edited. An absent NCBI Gene identifier on a score-bundle gene. |
| `ENSG00000157764` | `BRAF` | The worked example: symbol `BRAF`, `HGNC:1097`, NCBI Gene 673. |
| `ENSG00000167034` | `NKX3-1` | A hyphen in the approved symbol and a period in the alias `NKX3.1`. The placeholder guard passes both. 7,411 approved symbols in the release carry a hyphen. A guard that rejected a hyphen would unname every one of them. |
| `ENSG00000169129` | `AFAP1L2` | The score-bundle gene that raises a source-reference ambiguity. |
| `ENSG00000175658` | `DRD5P2`, `DRD5P3` | Two approved records for one accession. |
| `ENSG00000185974` | `AC092143.1` | Edited. A clone-derived placeholder symbol on a score-bundle gene. |

Two cells are edited. Every other byte is verbatim from the published release, including the quoting described above.

1. `ENSG00000141510` has its `entrez_id` cleared. HGNC publishes `7157` for TP53. The published release carries 590 accessions whose `entrez_id` is empty, and none of them is in the score bundle. The edit puts that case where a score lookup can reach it.
2. `ENSG00000185974` has its `symbol` replaced with `AC092143.1`. HGNC publishes `GRK1`. The 2026-09-04 release carries no clone-derived approved symbol at all, so the case cannot be drawn from real rows. The edit exercises the guard that withholds a placeholder name rather than reporting a string that reads like a symbol without being one.

`ENSG00000175727` is deliberately absent. It is a score-bundle gene and it raises a source-reference ambiguity, so a lookup for it reaches the naming source and finds nothing.

`ENSG00000169129` keeps its verbatim alias `Em:AC005383.4`. The placeholder guard reads the approved symbol. It does not reach alias symbols.
