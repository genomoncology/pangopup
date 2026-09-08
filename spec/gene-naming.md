# Gene naming

A score record identifies its gene by Ensembl accession. A naming source adds
the labels a consumer displays beside that accession. The accession stays the
identity. A symbol is a label. Symbols get renamed and accessions do not.

Two output surfaces gain names. Structured score records gain them, and so does
the gene reported on a source-reference ambiguity. The human-readable table does
not.

Previous symbols and alias symbols ship, and they are not identifiers. One alias
symbol can point at several genes, and an alias symbol can be another gene's
approved symbol. Never match on a previous or alias symbol alone. The Ensembl
accession remains the only key.

## Inspecting a naming source

The offline builder reports what a naming source yields, one line per Ensembl
accession, sorted by accession. It reports every value of a multi-valued field
rather than one pick, and it writes `-` where the source supplies nothing.

The source quotes a field that holds more than one value and leaves a single
value bare. The reader strips those quotes and reports the values themselves.
`CD4` carries the source cell `"T4|Leu-3"` and reports the two alias symbols
`T4` and `Leu-3`.

```bash
pangopup-build naming inspect ../tests/fixtures/gene-naming-mini/hgnc_complete_set_2026-09-04.tsv | mustmatch like "gene=ENSG00000010610 hgnc=HGNC:1678 symbol=CD4 ncbi=920 prev=- alias=T4|Leu-3
gene=ENSG00000119888 hgnc=HGNC:11529 symbol=EPCAM ncbi=4072 prev=M4S1|MIC18|TACSTD1 alias=Ly74|TROP1|GA733-2|EGP34|EGP40|EGP-2|KSA|CD326|Ep-CAM|HEA125|KS1/4|MK-1|MH99|MOC31|MOC-31|323/A3|17-1A|TACST-1|CO-17A|ESA|BerEp4|Ber-Ep4
gene=ENSG00000141499 hgnc=HGNC:25522 symbol=WRAP53 ncbi=55135 prev=WDR79 alias=FLJ10385|TCAB1
gene=ENSG00000141510 hgnc=HGNC:11998 symbol=TP53 ncbi=- prev=- alias=p53|LFS1
gene=ENSG00000157764 hgnc=HGNC:1097 symbol=BRAF ncbi=673 prev=- alias=BRAF1|BRAF-1
gene=ENSG00000167034 hgnc=HGNC:7838 symbol=NKX3-1 ncbi=4824 prev=NKX3A alias=NKX3.1|BAPX2
gene=ENSG00000169129 hgnc=HGNC:25901 symbol=AFAP1L2 ncbi=84632 prev=KIAA1914 alias=FLJ14564|Em:AC005383.4|XB130
gene=ENSG00000175658 unnamed=conflicting_records
gene=ENSG00000185974 unnamed=placeholder_symbol
total rows=10 accessions=9 named=7 unnamed=2"
```

`ENSG00000175658` carries two approved HGNC records, `DRD5P2` and `DRD5P3`. It
reports no name. Picking one record would be invisible to a consumer.
`ENSG00000230417` and `ENSG00000250413` carry the same conflict in the published
release.

`ENSG00000185974` carries a clone-derived string in place of a symbol. It
reports no name. The row is still read and the accession is still reported. The
guard withholds the name. It keeps the record that carries the placeholder.

An approved symbol never contains a period. All 45,045 approved records in the
2026-09-04 release satisfy that. A clone-derived name such as `AC092143.1`
carries the period of its accession version. The guard reads the approved
symbol alone and withholds the name when it finds a period there. The guard
reaches no other field. `ENSG00000169129` keeps its alias `Em:AC005383.4`.
`ENSG00000167034` keeps its alias `NKX3.1`. Both stay named.

A hyphen is ordinary in an approved symbol. 7,411 approved symbols in the
release carry one. `NKX3-1` is one of them and it reports its name.

## Installing a naming source

Installation is local and offline. It reads one dated release file and never
reaches the network. A score record carries no names until a naming source is
installed.

```bash
chmod -R u+w ../target/spec/gene-naming 2>/dev/null || true
rm -rf ../target/spec/gene-naming
mkdir -p ../target/spec/gene-naming
cp -R ../tests/fixtures/snv-regression/bundle ../target/spec/gene-naming/bundle
pangopup-build transport pack --bundle ../target/spec/gene-naming/bundle --output ../target/spec/gene-naming/transport >/dev/null
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup assets install --transport ../target/spec/gene-naming/transport --data-dir "$data" >/dev/null
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A > ../target/spec/gene-naming/before.jsonl
rg -F '"gene":"ENSG00000010610"' ../target/spec/gene-naming/before.jsonl >/dev/null
! rg -F 'gene_names' ../target/spec/gene-naming/before.jsonl
printf 'a score record carries no names before a naming source is installed\n' | mustmatch like 'a score record carries no names before a naming source is installed'
```

Install publishes one naming source and reports the release it belongs to.
Reinstalling the same source reuses it.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
source=../tests/fixtures/gene-naming-mini/hgnc_complete_set_2026-09-04.tsv
pangopup assets naming install --source "$source" --data-dir "$data" | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"installed","release":"hgnc-2026-09-04","source_bytes":6092,"source_sha256":"sha256:<digest>","named_genes":7'
pangopup assets naming install --source "$source" --data-dir "$data" | sed -E 's/sha256:[0-9a-f]{64}/sha256:<digest>/' | mustmatch like '{"status":"reused","release":"hgnc-2026-09-04","source_bytes":6092,"source_sha256":"sha256:<digest>","named_genes":7'
```

## Names on score records

A named gene reports its approved symbol, its HGNC identifier and its NCBI Gene
identifier beside the Ensembl accession it already reports. A multi-valued field
reports every value in source order.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A | rg -F '"gene_names":{"symbol":"CD4","hgnc_id":"HGNC:1678","ncbi_gene_id":920,"alias_symbols":["T4","Leu-3"]}' >/dev/null
pangopup lookup --data-dir "$data" --variant GRCh38:chr17:7686073:A:C | rg -F '"gene_names":{"symbol":"WRAP53","hgnc_id":"HGNC:25522","ncbi_gene_id":55135,"prev_symbols":["WDR79"],"alias_symbols":["FLJ10385","TCAB1"]}' >/dev/null
printf 'a named gene reports every symbol and identifier the source supplies\n' | mustmatch like 'a named gene reports every symbol and identifier the source supplies'
```

A field the naming source does not supply is absent. It is never empty and never
a placeholder.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr17:7687427:A:T | rg -F '"gene_names":{"symbol":"TP53","hgnc_id":"HGNC:11998","alias_symbols":["p53","LFS1"]}' >/dev/null
printf 'an unsupplied naming field is absent rather than empty\n' | mustmatch like 'an unsupplied naming field is absent rather than empty'
```

The human-readable table carries no names. It keeps its fifteen columns and
reports the Ensembl accession under `GENE`. A consumer reading names reads the
structured record.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A --format table > ../target/spec/gene-naming/table.txt
head -1 ../target/spec/gene-naming/table.txt | mustmatch like 'ASSEMBLY	CONTIG	POS	REF	ALT	STATUS	GENE	GAIN_SCORE	GAIN_POS	LOSS_SCORE	LOSS_POS	SOURCE_REF	PUBLISHED_ALTS	OMITTED_ALT	BUNDLE_ID'
rg -F 'ENSG00000010610' ../target/spec/gene-naming/table.txt >/dev/null
! rg -F 'CD4' ../target/spec/gene-naming/table.txt
! rg -F 'HGNC' ../target/spec/gene-naming/table.txt
printf 'the human-readable table gains no names\n' | mustmatch like 'the human-readable table gains no names'
```

A gene the naming source does not name reports the Ensembl accession alone. A
gene whose accession carries a placeholder symbol does the same.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:122093259:G:A > ../target/spec/gene-naming/absent.jsonl
rg -F '"gene":"ENSG00000175727"' ../target/spec/gene-naming/absent.jsonl >/dev/null
! rg -F 'gene_names' ../target/spec/gene-naming/absent.jsonl
pangopup lookup --data-dir "$data" --variant GRCh38:chr13:113673020:G:A > ../target/spec/gene-naming/placeholder.jsonl
rg -F '"gene":"ENSG00000185974"' ../target/spec/gene-naming/placeholder.jsonl >/dev/null
! rg -F 'gene_names' ../target/spec/gene-naming/placeholder.jsonl
printf 'an unnamed gene reports its accession and no name\n' | mustmatch like 'an unnamed gene reports its accession and no name'
```

A source-reference ambiguity names its gene on the same terms. It reports the
naming object when the source names the gene, and the accession alone when it
does not.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr10:114306066:A:C > ../target/spec/gene-naming/ambiguity.jsonl
rg -F '"status":"ambiguous_source_reference"' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
rg -F '"gene":"ENSG00000169129"' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
rg -F '"gene_names":{"symbol":"AFAP1L2","hgnc_id":"HGNC:25901","ncbi_gene_id":84632,"prev_symbols":["KIAA1914"],"alias_symbols":["FLJ14564","Em:AC005383.4","XB130"]}' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:122093260:T:A > ../target/spec/gene-naming/ambiguity-unnamed.jsonl
rg -F '"gene":"ENSG00000175727"' ../target/spec/gene-naming/ambiguity-unnamed.jsonl >/dev/null
! rg -F 'gene_names' ../target/spec/gene-naming/ambiguity-unnamed.jsonl
printf 'a source-reference ambiguity carries the same naming object\n' | mustmatch like 'a source-reference ambiguity carries the same naming object'
```

Naming is not a scoring fact. Installing a naming source changes no score,
no position, no status and no provenance.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A \
  | sed -E 's/,"gene_names":\{[^}]*\}//g' > ../target/spec/gene-naming/stripped.jsonl
cmp ../target/spec/gene-naming/before.jsonl ../target/spec/gene-naming/stripped.jsonl
printf 'installing a naming source changes no scoring byte\n' | mustmatch like 'installing a naming source changes no scoring byte'
```
