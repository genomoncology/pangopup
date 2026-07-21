# FAQ and Open Choices

## Settled explanations

### Is the downloaded archive the build input?

Yes. The extracted 19,913 `.tsv.gz` files already present under
`/home/ian/workspace/data/pangolin-precompute/` are the read-only input. The
builder will not redownload or commit them.

### Is this an exome-only lookup?

No. “Protein-coding genes” describes which gene spans were scored, not only the
protein-coding exon bases. The files cover complete gene spans, including
intronic positions. Exonic variants away from junctions can create cryptic
splice sites or alter splicing enhancers/silencers. Intronic variants can alter
branch-point, polypyrimidine, or other regulatory sequence and can create
cryptic sites or pseudoexons away from the canonical two splice-site bases.

### Why include a gene in the result?

The files are per gene, gene spans can overlap, and masking depends on annotation
context. One genomic SNV can therefore have more than one source record. Results
must retain Ensembl gene identity even if the common query has only one hit. A
caller does not need to supply a gene: without a filter Pangopup returns every
matching source record.

### What happens at a source row whose reference is `N`?

The complete source has 30 such loci. Pangopup preserves them so a rebuilt index
can account for every published row, but `N` is not a concrete SNV reference and
the source supplies only three of four possible alternates. Normal lookup returns
a typed ambiguous-source-reference result; it never guesses or silently maps the
row onto a pinned FASTA base.

### Why not simply Tabix the files?

Tabix is a good correctness and operational baseline. It still stores repeated
text-oriented keys, uses block decompression, and does not exploit the source’s
three-alternates-per-contiguous-locus structure. Pangopup should outperform it
by direct addressing, but measurements decide.

### Why not add an LRU cache immediately?

An mmap plus the operating-system page cache already retains hot pages. A second
cache adds memory, synchronization, and invalidation behavior. Add one only if
end-to-end measurements show repeated decoding or model execution that the page
cache cannot solve.

### Is the reference definitely GRCh38.p14?

The publisher says hg38. A local 1,023,901-position check across ten genes had
zero reference mismatches against RefSeq GRCh38.p14 primary chromosomes, but the
publisher does not identify the exact FASTA or GENCODE release. Pangopup can say
GRCh38 and pin the archive checksum; it should not invent missing provenance.

### Does Pangopup need Genome, HGVS, or transcript projection?

No. Pangopup is standalone and accepts an already identified GRCh38 genomic
variant: contig, one-based position, reference allele, and alternate allele.
That is enough to look up or model a splice score. Transcript `c.` and protein
`p.` expressions must be resolved by the caller because doing so requires a
general transcript/protein reference system and is not splice scoring.

### What reference and annotation data does model fallback need?

The lookup path needs only the sparse score bundle. The model path additionally
needs the model checkpoints, local GRCh38 DNA bases, and a map of gene strand
plus exon boundaries. The DNA is pinned NCBI RefSeq GRCh38.p14
`GCF_000001405.40`. The boundary map is compiled from the GENCODE annotation
used by Pangolin's masking behavior. It is a compact Pangopup mmap member, not
UTA, SeqRepo, Genome, SQLite, or gffutils at runtime.

### Why is any gene information needed at all?

The neural network needs only sequence and strand to produce raw changes, but
Pangolin's default masked result uses exon boundaries. It suppresses changes
that do not make biological sense relative to the annotated splice sites. It
also evaluates overlapping genes separately. Pangopup therefore needs only
gene ID, span, strand, and exon-boundary positions—not gene descriptions,
aliases, transcripts, proteins, or disease knowledge.

### Can the large files be GitHub release assets?

Yes. GitHub currently permits up to 1,000 assets per release, requires each
asset to be under 2 GiB, and states no aggregate release-size or bandwidth
quota. The measured direct sparse payload is about 1.589 GiB before small
directories and should be smaller as a transport archive. Pangopup will publish
separate verified executable, lookup-data, and model assets; installation
expands the data once so runtime lookup remains decompression-free.

## Settled product choices

### What does CLI v1 require?

Accept an explicit GRCh38 contig, position, reference, and alternate plus an
optional Ensembl source-gene filter. Without a filter, return every matching
gene-specific score. No implicit best-gene selection.

### How much HGVS does Pangopup own?

None beyond possibly recognizing an exact genomic RefSeq accession as a contig
alias. Pangopup does not accept transcript/protein HGVS and has no dependency on
Genome or another projection service.

### What corpus should prove the first index?

- **Recommended:** a checked-in miniature containing normal loci, both genomic
  directions, overlapping genes, zero scores, boundary positions, and malformed
  rows; then certify all 19,913 files before calling v1 complete.
- Start directly with the full 13 GB source: realistic, but slow feedback and
  poor failure isolation.
- Limit the product to the current ten clinical genes: fastest product proof,
  but creates a temporary scope and artifact that would soon be discarded.

### What is the primary optimization objective?

Exactness is mandatory and lookup speed is the primary optimization objective.
Download and installed size are secondary. The direct sparse mmap layout is the
baseline; compressed and fixed-width layouts remain measured comparators.

### How are large artifacts delivered?

As separately versioned GitHub release assets: executable, CC BY sparse lookup
bundle, GPL model weights, GRCh38 reference member, and GENCODE masking member.
Compress for transport, verify and expand once at installation, and map the
expanded data at runtime.

## Remaining design choices

### What should the first output look like?

- **Recommended:** JSON Lines for stable machine use plus a concise human table.
- JSON only: simplest contract, less pleasant for direct inspection.
- Tabular only: convenient interactively, fragile for downstream integration.
