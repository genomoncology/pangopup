# Miniature gene-naming source

`hgnc_complete_set_2026-09-04.tsv` is an eleven-row excerpt of the HGNC complete set, monthly release 2026-09-04. It keeps the full 53-column header so it parses exactly like the published file. It carries the published file name because it belongs to that release. Its own size and digest are its own. Only the fetched production file matches the published pin below.

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
| `ENSG05220017861` | `DDX11L3` | An accession whose numeric part is 5,220,017,861. It does not fit in 32 bits. The index key must be wide enough to carry it without truncating. |

Two cells are edited. Every other byte is verbatim from the published release, including the quoting described above.

1. `ENSG00000141510` has its `entrez_id` cleared. HGNC publishes `7157` for TP53. The published release carries 590 accessions whose `entrez_id` is empty, and none of them is in the score bundle. The edit puts that case where a score lookup can reach it.
2. `ENSG00000185974` has its `symbol` replaced with `AC092143.1`. HGNC publishes `GRK1`. The 2026-09-04 release carries no clone-derived approved symbol at all, so the case cannot be drawn from real rows. The edit exercises the guard that withholds a placeholder name rather than reporting a string that reads like a symbol without being one.

`ENSG00000175727` is deliberately absent. It is a score-bundle gene and it raises a source-reference ambiguity, so a lookup for it reaches the naming source and finds nothing.

`ENSG00000169129` keeps its verbatim alias `Em:AC005383.4`. The placeholder guard reads the approved symbol. It does not reach alias symbols.

## Miniature NCBI source

`Homo_sapiens.gene_info` is a nine-row excerpt of the NCBI gene information file, fetched 2026-09-08. It keeps the full 16-column header so it parses exactly like the published file. The published file is gzipped and this excerpt is not, because the reader detects the gzip magic and reads either form.

Source: NCBI (National Center for Biotechnology Information), U.S. National Library of Medicine.
File: `Homo_sapiens.gene_info.gz`.
URL: <https://ftp.ncbi.nlm.nih.gov/gene/DATA/GENE_INFO/Mammalia/Homo_sapiens.gene_info.gz>
Fetched size: 5180589 bytes.

NCBI replaces this file at a fixed URL and publishes no dated archive of it. A digest recorded today cannot be re-fetched tomorrow. The built index is the durable artifact and the repository carries it.

| Accession | Row | Why it is here |
| --- | --- | --- |
| `ENSG00000010610` | `CD4` | HGNC also names it. NCBI lists five synonyms and HGNC lists two alias symbols. The merged record must report HGNC's two. |
| — | `CYP2D7BP` | A row whose `dbXrefs` cell is `-`. It carries no Ensembl accession and the index must not reach it. |
| `ENSG00000185974` | `GRK1` | HGNC reaches this accession and withholds the name because the fixture's HGNC symbol is a clone-derived placeholder. NCBI names it `GRK1`. The merged record must stay unnamed. |
| `ENSG00000141510` | `TP53` | HGNC reaches it with an empty `entrez_id`. NCBI carries `7157`. The merged record must report no NCBI Gene identifier. |
| `ENSG00000235059` | `PRY`, `LOC101929148` | Two NCBI records for one accession, and HGNC does not reach it. The merged record must report no name. |
| `ENSG00000233887` | `ERVFC1` | Named only by NCBI, with two synonyms and no HGNC cross-reference. |
| `ENSG00000249624` | `IFNAR2-IL10RB` | Named only by NCBI, with no synonyms. A readthrough locus HGNC does not name. |
| — | `Nkx3-1` | Edited. One fabricated row carrying `tax_id` 10090 and the human `NKX3-1` accession. The published human file carries no other organism, so the guard that admits only `tax_id` 9606 cannot be exercised from real rows. |

Every other byte is verbatim from the fetched file.
