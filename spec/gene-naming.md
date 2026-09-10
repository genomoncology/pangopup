# Gene naming

A score record identifies its gene by Ensembl accession. Gene names are the
labels a consumer displays beside that accession. The accession stays the
identity. A symbol is a label. Symbols get renamed and accessions do not.

The names ship with the build. PangoPup carries one gene-name index in the
repository and reads it from the executable. There is no naming asset to
install, no naming state under the data directory, and no way for a deployment
to vary the names it returns. A consumer refreshes gene names by upgrading
PangoPup.

Two output surfaces carry names. Structured score records carry them, and so
does the gene reported on a source-reference ambiguity. The human-readable
table does not.

Previous symbols and alias symbols ship, and they are not identifiers. One
alias symbol can point at several genes, and an alias symbol can be another
gene's approved symbol. Never match on a previous or alias symbol alone. The
Ensembl accession remains the only key.

The published compatibility document says the same thing. It states that one
index ships with the build, and it describes no status field a deployment can
read for an installed vintage.

```bash
inventory=$(awk '/^## v0.5.0 response-shape inventory$/ { on=1; next } on && /^## / { exit } on' ../architecture/compatibility.md)
printf '%s' "$inventory" | rg -F -- 'PangoPup ships one gene-name index with the build.' >/dev/null
printf '%s' "$inventory" | ../scripts/spec-refutes.sh --absent -F -- 'Status `naming` object'
printf '%s' "$inventory" | ../scripts/spec-refutes.sh --absent -F -- '`naming.available`'
printf '%s' "$inventory" | ../scripts/spec-refutes.sh --absent -F -- '`naming.release`'
printf 'the published inventory describes no installed naming vintage\n' | mustmatch like 'the published inventory describes no installed naming vintage'
```

## Two sources and one record

HGNC is the naming authority. Where HGNC reaches an accession, HGNC supplies
that accession's whole record and NCBI is not consulted for it. NCBI names
only the accessions HGNC does not reach. The two sources are never merged
field by field.

Every named record says which source named it. `source` reports `hgnc` or
`ncbi`. A gene NCBI named carries no HGNC identifier, because no HGNC record
names that accession, and its symbol is not HGNC-approved.

An accession that carries more than one record in the source naming it reports
no name. Picking one record would be invisible to a consumer.

The offline builder reports what the shipped index holds for the accessions a
maintainer names, followed by one total line.

```bash
pangopup-build naming inspect --index ../assets/gene-names/gene-names.pgn \
  ENSG00000157764 ENSG00000205702 ENSG00000233887 ENSG00000249624 ENSG00000175658 ENSG00000235059 ENSG00000002079 \
  | mustmatch like "gene=ENSG00000157764 source=hgnc hgnc=HGNC:1097 symbol=BRAF ncbi=673 prev=- alias=BRAF1|BRAF-1
gene=ENSG00000205702 source=hgnc hgnc=HGNC:2624 symbol=CYP2D7 ncbi=1564 prev=CYP2D|CYP2D@|CYP2D7P1|CYP2D7P alias=-
gene=ENSG00000233887 source=ncbi hgnc=- symbol=ERVFC1 ncbi=105373297 prev=- alias=Fc1env|HERV-Fc1env
gene=ENSG00000249624 source=ncbi hgnc=- symbol=IFNAR2-IL10RB ncbi=127882475 prev=- alias=-
gene=ENSG00000175658 unnamed=conflicting_records
gene=ENSG00000235059 unnamed=conflicting_records
gene=ENSG00000002079 absent"
```

`ENSG00000205702` shows the arbitration rule at work. HGNC approves `CYP2D7`
and reports NCBI Gene 1564. NCBI reports `LOC105377203` and gene 105377203 for
the same accession. HGNC names it, so the whole record comes from HGNC.

`ENSG00000233887` and `ENSG00000249624` are named only by NCBI. Both report no
HGNC identifier. `ERVFC1` keeps the two synonyms NCBI publishes as alias
symbols. NCBI publishes no previous-symbol field, so an NCBI-named gene reports
no previous symbols.

`ENSG00000175658` carries two approved HGNC records. `ENSG00000235059` carries
two NCBI records and no HGNC record. Both report no name.

`ENSG00000002079` is a scoreable gene that neither source reaches. The index
holds nothing for it.

## Names on score records

A named gene reports its approved symbol, the source that named it, and the
identifiers that source supplies, beside the Ensembl accession it already
reports. A multi-valued field reports every value in source order. No install
step precedes any of this.

```bash
chmod -R u+w ../target/spec/gene-naming 2>/dev/null || true
rm -rf ../target/spec/gene-naming
mkdir -p ../target/spec/gene-naming
cp -R ../tests/fixtures/snv-regression/bundle ../target/spec/gene-naming/bundle
pangopup-build transport pack --bundle ../target/spec/gene-naming/bundle --output ../target/spec/gene-naming/transport >/dev/null
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup assets install --transport ../target/spec/gene-naming/transport --data-dir "$data" >/dev/null
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A > ../target/spec/gene-naming/named.jsonl
rg -F '"gene_names":{"symbol":"CD4","source":"hgnc","hgnc_id":"HGNC:1678","ncbi_gene_id":920,"alias_symbols":["T4","Leu-3"]}' ../target/spec/gene-naming/named.jsonl >/dev/null
pangopup lookup --data-dir "$data" --variant GRCh38:chr17:7686073:A:C | rg -F '"gene_names":{"symbol":"WRAP53","source":"hgnc","hgnc_id":"HGNC:25522","ncbi_gene_id":55135,"prev_symbols":["WDR79"],"alias_symbols":["FLJ10385","TCAB1"]}' >/dev/null
printf 'a named gene reports its source, its symbols and its identifiers\n' | mustmatch like 'a named gene reports its source, its symbols and its identifiers'
```

A field the naming source does not supply is absent. It is never empty and
never a placeholder.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr17:7687427:A:T | rg -F '"gene_names":{"symbol":"TP53","source":"hgnc","hgnc_id":"HGNC:11998","ncbi_gene_id":7157,"alias_symbols":["p53","LFS1"]}' >/dev/null
printf 'an unsupplied naming field is absent rather than empty\n' | mustmatch like 'an unsupplied naming field is absent rather than empty'
```

A source-reference ambiguity names its gene on the same terms.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr10:114306066:A:C > ../target/spec/gene-naming/ambiguity.jsonl
rg -F '"status":"ambiguous_source_reference"' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
rg -F '"gene":"ENSG00000169129"' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
rg -F '"gene_names":{"symbol":"AFAP1L2","source":"hgnc","hgnc_id":"HGNC:25901","ncbi_gene_id":84632,"prev_symbols":["KIAA1914"],"alias_symbols":["FLJ14564","Em:AC005383.4","XB130"]}' ../target/spec/gene-naming/ambiguity.jsonl >/dev/null
printf 'a source-reference ambiguity carries the same naming object\n' | mustmatch like 'a source-reference ambiguity carries the same naming object'
```

The human-readable table carries no names. It keeps its fifteen columns and
reports the Ensembl accession under `GENE`. A consumer reading names reads the
structured record.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A --format table > ../target/spec/gene-naming/table.txt
head -1 ../target/spec/gene-naming/table.txt | mustmatch like 'ASSEMBLY	CONTIG	POS	REF	ALT	STATUS	GENE	GAIN_SCORE	GAIN_POS	LOSS_SCORE	LOSS_POS	SOURCE_REF	PUBLISHED_ALTS	OMITTED_ALT	BUNDLE_ID'
rg -F 'ENSG00000010610' ../target/spec/gene-naming/table.txt >/dev/null
../scripts/spec-refutes.sh --absent -F -- 'CD4' ../target/spec/gene-naming/table.txt
../scripts/spec-refutes.sh --absent -F -- 'HGNC' ../target/spec/gene-naming/table.txt
printf 'the human-readable table gains no names\n' | mustmatch like 'the human-readable table gains no names'
```

Naming is not a scoring fact. Stripping every naming object leaves a byte-identical scoring record.

```bash
data=$(cd .. && pwd)/target/spec/gene-naming/data
pangopup lookup --data-dir "$data" --variant GRCh38:chr12:6801301:G:A \
  | sed -E 's/,"gene_names":\{[^}]*\}//g' > ../target/spec/gene-naming/stripped.jsonl
rg -F '"gene":"ENSG00000010610"' ../target/spec/gene-naming/stripped.jsonl >/dev/null
../scripts/spec-refutes.sh --absent -F -- 'gene_names' ../target/spec/gene-naming/stripped.jsonl
printf 'a naming object carries no scoring byte\n' | mustmatch like 'a naming object carries no scoring byte'
```

## Building the index

`make gene-name-index` downloads both sources, builds the index, writes it to
its committed path, and reports the source digests and row counts it built
from. That target reaches the network and a maintainer runs it by hand. No
gate invokes it, and no gate reaches the network.

```bash
rg -F 'gene-name-index:' ../Makefile >/dev/null
rg -F 'assets/gene-names/gene-names.pgn' ../Makefile >/dev/null
awk '/^lint:/,/^$/' ../Makefile | ../scripts/spec-refutes.sh --absent -F -- 'gene-name-index'
awk '/^test:/,/^$/' ../Makefile | ../scripts/spec-refutes.sh --absent -F -- 'gene-name-index'
awk '/^spec:/,/^$/' ../Makefile | ../scripts/spec-refutes.sh --absent -F -- 'gene-name-index'
printf 'the download target exists and no gate invokes it\n' | mustmatch like 'the download target exists and no gate invokes it'
```

The repository records what the committed index was built from beside it.
`assets/gene-names/provenance.json` names each source, its publisher, its URL,
its byte count, its SHA-256 and its row count, and it states whether those
bytes can be fetched again. HGNC publishes immutable dated monthly releases.
NCBI replaces `Homo_sapiens.gene_info.gz` at a fixed URL and publishes no dated
archive of it, so the recorded NCBI bytes cannot be fetched again. The built
index is the durable artifact.

```bash
python3 -c "
import json, hashlib, pathlib
record = json.loads(pathlib.Path('../assets/gene-names/provenance.json').read_text())
member = pathlib.Path('../assets/gene-names/gene-names.pgn').read_bytes()
assert record['index']['bytes'] == len(member)
assert record['index']['sha256'] == 'sha256:' + hashlib.sha256(member).hexdigest()
assert [source['refetchable'] for source in record['sources']] == [True, False]
"
printf 'the record beside the index matches the index it describes\n' | mustmatch like 'the record beside the index matches the index it describes'
```

Rebuilding the index from identical source bytes produces identical index
bytes. The builder sorts every section, packs the string runs with no padding,
and writes no timestamp. `cargo test` proves it against two miniature source
excerpts in the checkout, so the proof reaches no network.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-build \
  --test gene_name_index \
  rebuilding_from_identical_source_bytes_produces_identical_index_bytes >/dev/null
printf 'the builder is deterministic and no gate reaches the network\n' | mustmatch like 'the builder is deterministic and no gate reaches the network'
```

The index answers one accession from a bounded number of memory pages, so the
cost of a name does not grow with the number of genes the index holds.

```bash
../scripts/spec-cargo-test.sh 1 --locked --quiet --package pangopup-index \
  --test gene_names \
  resolving_one_gene_name_addresses_a_bounded_number_of_pages >/dev/null
printf 'one accession costs a bounded number of index pages\n' | mustmatch like 'one accession costs a bounded number of index pages'
```
